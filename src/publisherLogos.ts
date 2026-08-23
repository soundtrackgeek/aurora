export interface PublisherLogoOverride {
  dataUrl: string;
  updatedAt: string;
}

export type PublisherLogoOverrides = Record<string, PublisherLogoOverride>;

const STORAGE_KEY = "aurora.publisher-logo-overrides.v1";
const STORAGE_VERSION = 1;
const MAX_SOURCE_BYTES = 5 * 1024 * 1024;
const MAX_OVERRIDES = 48;
const MAX_STORED_CHARACTERS = 3_500_000;
const OUTPUT_SIZE = 192;
const OUTPUT_PADDING = 14;
const ALLOWED_IMAGE_TYPES = new Set(["image/jpeg", "image/png", "image/webp"]);
const ALLOWED_DATA_URL = /^data:image\/(?:jpeg|png|webp);base64,/i;
const IGNORED_MONOGRAM_WORDS = new Set([
  "and",
  "group",
  "label",
  "ltd",
  "music",
  "of",
  "record",
  "recordings",
  "records",
  "the",
]);

interface StoredPublisherLogos {
  version: 1;
  logos: PublisherLogoOverrides;
}

export function publisherLogoKey(publisher: string) {
  return publisher.normalize("NFKC").trim().replace(/\s+/g, " ").toLowerCase();
}

export function publisherMonogram(publisher: string) {
  const words = publisher.normalize("NFKC").match(/[\p{L}\p{N}]+/gu) ?? [];
  const meaningful = words.filter((word) => !IGNORED_MONOGRAM_WORDS.has(word.toLowerCase()));
  const candidates = meaningful.length ? meaningful : words;
  if (!candidates.length) return "?";
  if (candidates.length === 1) {
    const word = candidates[0];
    if (word === word.toUpperCase() && word.length <= 4) return word;
    return word.slice(0, 1).toUpperCase();
  }
  return candidates.slice(0, 3).map((word) => word.slice(0, 1).toUpperCase()).join("");
}

export function publisherLogoVariant(publisher: string) {
  let hash = 2166136261;
  for (const character of publisherLogoKey(publisher)) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return Math.abs(hash) % 6;
}

function logoStorage() {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

function validOverride(value: unknown): value is PublisherLogoOverride {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<PublisherLogoOverride>;
  return typeof candidate.dataUrl === "string"
    && candidate.dataUrl.length <= MAX_STORED_CHARACTERS
    && ALLOWED_DATA_URL.test(candidate.dataUrl)
    && typeof candidate.updatedAt === "string";
}

export function loadPublisherLogoOverrides(): PublisherLogoOverrides {
  const storage = logoStorage();
  if (!storage) return {};
  try {
    const parsed = JSON.parse(storage.getItem(STORAGE_KEY) ?? "null") as Partial<StoredPublisherLogos> | null;
    if (parsed?.version !== STORAGE_VERSION || !parsed.logos || typeof parsed.logos !== "object") return {};
    return Object.fromEntries(Object.entries(parsed.logos).filter(([, value]) => validOverride(value)));
  } catch {
    return {};
  }
}

function persistPublisherLogoOverrides(logos: PublisherLogoOverrides) {
  const storage = logoStorage();
  if (!storage) throw new Error("Device-local logo storage is unavailable.");
  const payload: StoredPublisherLogos = { version: STORAGE_VERSION, logos };
  const serialized = JSON.stringify(payload);
  if (serialized.length > MAX_STORED_CHARACTERS) {
    throw new Error("The local publisher-logo library is full. Clear an existing override and try again.");
  }
  try {
    storage.setItem(STORAGE_KEY, serialized);
  } catch {
    throw new Error("Aurora could not save this logo in device-local storage.");
  }
}

export function savePublisherLogoOverride(
  current: PublisherLogoOverrides,
  publisher: string,
  dataUrl: string,
): PublisherLogoOverrides {
  if (!ALLOWED_DATA_URL.test(dataUrl)) throw new Error("Choose a PNG, JPEG, or WebP image.");
  const key = publisherLogoKey(publisher);
  if (!(key in current) && Object.keys(current).length >= MAX_OVERRIDES) {
    throw new Error("Aurora can keep up to 48 local publisher-logo overrides on this device.");
  }
  const next = { ...current, [key]: { dataUrl, updatedAt: new Date().toISOString() } };
  persistPublisherLogoOverrides(next);
  return next;
}

export function clearPublisherLogoOverride(
  current: PublisherLogoOverrides,
  publisher: string,
): PublisherLogoOverrides {
  const key = publisherLogoKey(publisher);
  if (!(key in current)) return current;
  const next = { ...current };
  delete next[key];
  persistPublisherLogoOverrides(next);
  return next;
}

function loadImage(file: File) {
  return new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    const objectUrl = URL.createObjectURL(file);
    image.onload = () => {
      URL.revokeObjectURL(objectUrl);
      resolve(image);
    };
    image.onerror = () => {
      URL.revokeObjectURL(objectUrl);
      reject(new Error("Aurora could not read that image."));
    };
    image.src = objectUrl;
  });
}

export async function preparePublisherLogo(file: File) {
  if (!ALLOWED_IMAGE_TYPES.has(file.type)) throw new Error("Choose a PNG, JPEG, or WebP image.");
  if (file.size > MAX_SOURCE_BYTES) throw new Error("Publisher logos must be 5 MB or smaller.");
  const image = await loadImage(file);
  if (!image.naturalWidth || !image.naturalHeight) throw new Error("Aurora could not read that image.");
  const canvas = document.createElement("canvas");
  canvas.width = OUTPUT_SIZE;
  canvas.height = OUTPUT_SIZE;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Aurora could not prepare that image.");
  context.imageSmoothingEnabled = true;
  context.imageSmoothingQuality = "high";
  const available = OUTPUT_SIZE - OUTPUT_PADDING * 2;
  const scale = Math.min(available / image.naturalWidth, available / image.naturalHeight);
  const width = image.naturalWidth * scale;
  const height = image.naturalHeight * scale;
  context.drawImage(image, (OUTPUT_SIZE - width) / 2, (OUTPUT_SIZE - height) / 2, width, height);
  const webp = canvas.toDataURL("image/webp", .9);
  const dataUrl = webp.startsWith("data:image/webp") ? webp : canvas.toDataURL("image/png");
  if (!ALLOWED_DATA_URL.test(dataUrl) || dataUrl.length > 300_000) {
    throw new Error("That image remains too large after resizing. Choose a simpler logo image.");
  }
  return dataUrl;
}
