"""Train Aurora's sound descriptors (`sound:` and `group:sound`) from CLAP embeddings.

Music Library stores one 512-value LAION CLAP embedding per analyzed MP3 in
`music-clap.sqlite3`. CLAP puts text and audio in one space, but text-to-audio
scores carry a different bias per label ("orchestral" scores high on almost
everything), so they are not used to classify directly. Instead:

1. Each label's prompts are embedded with Music Library's CLAP text encoder.
2. Every score-genre track is scored against every label, z-scored per label,
   and the tracks where one label beats every other by the widest margin become
   that label's seeds (pseudo-labels).
3. One logistic regression per label is trained on the seeds' audio
   embeddings: its seeds against every other label's seeds.

Aurora only ships the result: 12 x 512 weights and 12 biases, so tagging a
track is 12 dot products on an embedding Music Library already computed.

Usage (needs numpy and scikit-learn, Music Library's CLAP worker and model):

    python -I scripts/train-sound-descriptors.py <music-library-data-dir> <clap-worker.exe>

It writes src-tauri/src/sound_descriptors_model.json. Text embeddings are
cached next to this script's output as sound-descriptors-text.json (ignored).
"""
import base64, json, os, sqlite3, struct, subprocess, sys
import numpy as np
from sklearn.linear_model import LogisticRegression

VERSION = "sound-v1"
CLAP_MODEL = "clap-lmas-e9fd5ac-3x10s-v1"
MODEL_FOLDER = "clap-model/larger_clap_music_and_speech-e9fd5ac"
THRESHOLD = 0.4
ALBUM_SHARE = 1 / 3
REGULARIZATION = 0.3
DEFAULT_SEEDS = 2000
SCORE_GENRES = ("action", "animation", "comedy", "documentary", "drama", "fantasy", "horror",
                "sci-fi", "thriller", "tv", "video game", "western", "anime")

# (id, display name, seeds, prompts). Order is the bit order in Aurora's cache.
LABELS = [
    ("orchestral", "Orchestral", DEFAULT_SEEDS, ["a full symphony orchestra playing a film score", "orchestral music with strings, brass and woodwinds", "a sweeping orchestral soundtrack"]),
    ("choir", "Choir", DEFAULT_SEEDS, ["a large choir singing", "epic choral music with many voices", "a church choir singing"]),
    ("strings", "Strings", DEFAULT_SEEDS, ["a small string ensemble", "a string quartet", "intimate strings playing slowly"]),
    ("piano", "Piano", DEFAULT_SEEDS, ["solo piano", "a single piano playing alone", "quiet solo piano piece"]),
    ("guitar", "Guitar", DEFAULT_SEEDS, ["acoustic guitar", "electric guitar", "solo guitar playing"]),
    ("synth", "Synth", DEFAULT_SEEDS, ["analog synthesizer music", "an electronic synth score with arpeggios", "vintage 1980s analog synthesizer film score", "slow atmospheric synthesizer pads and a synth melody"]),
    ("hybrid", "Hybrid", DEFAULT_SEEDS, ["orchestra combined with electronic beats and synthesizers", "hybrid orchestral electronic trailer music", "modern blockbuster score with orchestra and synth bass"]),
    ("ambient", "Ambient", DEFAULT_SEEDS, ["dark ambient drone", "atmospheric ambient soundscape", "eerie sustained drones and textures"]),
    ("percussion", "Percussion", DEFAULT_SEEDS, ["pounding action drums and percussion", "loud taiko drums", "driving percussion for an action scene"]),
    # Rare in scores: more seeds drift into lo-fi analog synth.
    ("chiptune", "Chiptune", 800, ["8-bit chiptune video game music", "retro video game music from a sound chip", "16-bit console game music"]),
    ("jazz", "Jazz", DEFAULT_SEEDS, ["jazz with saxophone and double bass", "a jazz band", "big band jazz"]),
    ("vocals", "Vocals", DEFAULT_SEEDS, ["a singer's voice singing a melody", "a solo female vocalist singing", "a song with a lead singer"]),
]


def text_embeddings(worker, model_dir, cache_path):
    try:
        cache = json.load(open(cache_path, encoding="utf-8"))
    except FileNotFoundError:
        cache = {}
    for _, _, _, prompts in LABELS:
        for prompt in prompts:
            if prompt in cache:
                continue
            run = subprocess.run([worker, "--text", model_dir], input=prompt.encode(), capture_output=True, check=True)
            raw = base64.b64decode(json.loads(run.stdout)["embedding"])
            cache[prompt] = list(struct.unpack(f"<{len(raw) // 4}f", raw))
            print("embedded:", prompt, flush=True)
    json.dump(cache, open(cache_path, "w", encoding="utf-8"))
    return cache


def score_embeddings(data_dir):
    c = sqlite3.connect(f"file:{os.path.join(data_dir, 'music-library.sqlite3')}?mode=ro", uri=True)
    c.execute("ATTACH ? AS a", (f"file:{os.path.join(data_dir, 'music-analysis.sqlite3')}?mode=ro",))
    c.execute("ATTACH ? AS clap", (f"file:{os.path.join(data_dir, 'music-clap.sqlite3')}?mode=ro",))
    rows = c.execute(f"""
        SELECT ca.embedding FROM tracks t
        JOIN a.sonic_tracks s ON s.directory = t.file_path AND s.filename = t.filename
        JOIN clap.clap_audio ca ON ca.audio_hash = s.audio_hash AND ca.model = ? AND ca.embedding IS NOT NULL
        WHERE LOWER(t.canonical_genre) IN ({",".join("?" * len(SCORE_GENRES))})""",
        (CLAP_MODEL, *SCORE_GENRES)).fetchall()
    return np.stack([np.frombuffer(r[0], dtype="<f2").astype(np.float32) for r in rows])


def main():
    data_dir, worker = sys.argv[1], sys.argv[2]
    out = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "src", "sound_descriptors_model.json")
    text = text_embeddings(worker, os.path.join(data_dir, MODEL_FOLDER),
                           os.path.join(os.path.dirname(out), "sound-descriptors-text.json"))
    emb = score_embeddings(data_dir)
    print("score tracks:", len(emb))

    def label_vector(prompts):
        v = np.mean([np.array(text[p]) / np.linalg.norm(text[p]) for p in prompts], axis=0)
        return v / np.linalg.norm(v)

    L = np.stack([label_vector(prompts) for *_, prompts in LABELS])
    S = emb @ L.T
    Z = (S - S.mean(0)) / S.std(0)
    seeds = []
    for i, (_, _, count, _) in enumerate(LABELS):
        margin = Z[:, i] - np.delete(Z, i, axis=1).max(1)
        seeds.append(np.argsort(-margin)[:count])
    labels = []
    for i, (label_id, name, count, prompts) in enumerate(LABELS):
        positive = seeds[i]
        negative = np.setdiff1d(np.unique(np.concatenate([s for j, s in enumerate(seeds) if j != i])), positive)
        X = np.concatenate([emb[positive], emb[negative]])
        y = np.r_[np.ones(len(positive)), np.zeros(len(negative))]
        model = LogisticRegression(C=REGULARIZATION, class_weight="balanced", max_iter=2000).fit(X, y)
        labels.append({
            "id": label_id, "name": name, "prompts": prompts, "seeds": int(count),
            "bias": round(float(model.intercept_[0]), 6),
            "weights": [round(float(w), 6) for w in model.coef_[0]],
        })
    P = 1 / (1 + np.exp(-(emb @ np.array([l["weights"] for l in labels]).T + np.array([l["bias"] for l in labels]))))
    share = {l["id"]: round(float((P[:, i] >= THRESHOLD).mean()), 4) for i, l in enumerate(labels)}
    print("share of score tracks per label:", share)
    json.dump({
        "version": VERSION, "clapModel": CLAP_MODEL, "dimensions": int(emb.shape[1]),
        "threshold": THRESHOLD, "albumShare": round(ALBUM_SHARE, 6),
        "training": {"scoreTracks": int(len(emb)), "regularization": REGULARIZATION, "scoreTrackShare": share},
        "labels": labels,
    }, open(out, "w", encoding="utf-8"), separators=(",", ":"))
    print("wrote", os.path.abspath(out))


if __name__ == "__main__":
    main()
