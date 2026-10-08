import { act, cleanup, fireEvent, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { usePlaylistsDomain } from "./usePlaylistsDomain";
import { browserPreview } from "../../library";
import { listMusicLibraryPlaylists, loadMusicLibraryPlaylist } from "../../playlists";
import type { PlaybackSnapshot } from "../../playback";

vi.mock("../../playlists",()=>({listMusicLibraryPlaylists:vi.fn(),loadMusicLibraryPlaylist:vi.fn(),loadSelectedPlaylistId:()=>null,saveSelectedPlaylistId:vi.fn()}));
afterEach(()=>{cleanup();vi.clearAllMocks();});

it("continues a paged queue with revision and shuffle guards, then surfaces a stale snapshot",async()=>{
  const tracks=browserPreview.tracks.slice(0,2);
  const snapshot={queue:tracks,currentIndex:0,shuffle:false} as PlaybackSnapshot;
  const append=vi.fn().mockResolvedValue({...snapshot,queue:[...tracks,tracks[1]]});
  const options:Parameters<typeof usePlaylistsDomain>[0]={playback:{state:{queue:[],currentIndex:null,shuffle:false},play:vi.fn().mockResolvedValue(snapshot),setShuffle:vi.fn()},appendPlayback:append,libraryReady:false,endGenreQueue:vi.fn(),selectTrack:vi.fn()};
  const {result,rerender}=renderHook(opts=>usePlaylistsDomain(opts),{initialProps:options});
  vi.mocked(loadMusicLibraryPlaylist).mockResolvedValue({id:3,name:"Shared",description:"",trackCount:200,missingCount:0,tracks:[tracks[1]],nextCursor:199,revision:"v1"});
  await act(async()=>{await result.current.startPlaylistQueue(tracks,0,false,{id:3,cursor:99,revision:"v1",shuffleSeed:42});});
  rerender({...options,playback:{...options.playback,state:snapshot}});
  await waitFor(()=>expect(append).toHaveBeenCalledWith([tracks[1]]));
  expect(loadMusicLibraryPlaylist).toHaveBeenCalledWith(3,99,"v1",42);
  vi.mocked(loadMusicLibraryPlaylist).mockRejectedValue(new Error("Playlist changed. Refresh."));
  await act(async()=>{await Promise.resolve();});
  rerender({...options,playback:{...options.playback,state:{...snapshot,queue:[...tracks,tracks[1]],currentIndex:1}}});
  await waitFor(()=>expect(result.current.playlistPlaybackError).toContain("Playlist changed"));
  expect(append).toHaveBeenCalledTimes(1);
});

it("reloads cross-app changes on focus while retaining the selected playlist",async()=>{
  vi.mocked(listMusicLibraryPlaylists).mockResolvedValue([{id:4,name:"Shared",description:"",trackCount:1,updatedAt:"v1"}]);
  const {result}=renderHook(()=>usePlaylistsDomain({playback:{state:{queue:[],currentIndex:null,shuffle:false},play:vi.fn(),setShuffle:vi.fn()},appendPlayback:vi.fn(),libraryReady:true,endGenreQueue:vi.fn(),selectTrack:vi.fn()}));
  await waitFor(()=>expect(result.current.selectedPlaylistId).toBe(4));
  vi.mocked(listMusicLibraryPlaylists).mockResolvedValue([{id:4,name:"Renamed in Music Library",description:"",trackCount:2,updatedAt:"v2"}]);
  fireEvent.focus(window);
  await waitFor(()=>expect(result.current.savedPlaylists[0].trackCount).toBe(2));
  expect(result.current.selectedPlaylistId).toBe(4);
});
