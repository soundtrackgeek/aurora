import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { authorPlaylist, listMusicLibraryPlaylists } from "../../playlists";
import type { Track } from "../../library";
import { AddToPlaylistButton, PlaylistAuthoringDialog, PlaylistRow } from "./PlaylistAuthoring";
import { PlaylistAuthoringContext } from "./playlistAuthoringContext";
vi.mock("../../playlists",()=>({authorPlaylist:vi.fn(),listMusicLibraryPlaylists:vi.fn()}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
Object.defineProperty(HTMLDialogElement.prototype,"showModal",{configurable:true,value:function(this:HTMLDialogElement){this.setAttribute("open","");}});
const song={id:"1",trackKey:"key",title:"Song"} as Track;

it("captures queue repeats and appends only to regular playlists using their displayed revision",async()=>{
  vi.mocked(listMusicLibraryPlaylists).mockResolvedValue([{id:2,name:"Regular",description:"",trackCount:1,updatedAt:"rev",editable:true},{id:3,name:"Smart",description:"",trackCount:1,updatedAt:"rev",smart:true,editable:false}]);
  vi.mocked(authorPlaylist).mockResolvedValue({id:2});
  const saved=vi.fn();render(<PlaylistAuthoringDialog selection={{tracks:[song,song],label:"Queue"}} onClose={vi.fn()} onSaved={saved} />);
  await screen.findByRole("option",{name:"Regular"});
  expect(screen.queryByRole("option",{name:"Smart"})).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Playlist"),{target:{value:"2"}});
  fireEvent.click(screen.getByRole("button",{name:"Add songs"}));
  await waitFor(()=>expect(authorPlaylist).toHaveBeenCalledWith({action:"append",id:2,expectedUpdatedAt:"rev",name:undefined,tracks:[song,song],albumIds:undefined}));
  expect(saved).toHaveBeenCalledWith(2);
});

it("opens row actions without selecting or playing the row and supports keyboard context access",()=>{
  const open=vi.fn();const row=vi.fn();
  render(<PlaylistAuthoringContext value={open}><PlaylistRow selection={{tracks:[song],label:"Song"}}><div role="row" onClick={row} onKeyDown={row}><AddToPlaylistButton track={song} label="Song" /></div></PlaylistRow></PlaylistAuthoringContext>);
  fireEvent.click(screen.getByRole("button",{name:"Add Song to playlist"}));
  expect(open).toHaveBeenCalledWith({tracks:[song],albumIds:undefined,label:"Song"});expect(row).not.toHaveBeenCalled();
  fireEvent.keyDown(screen.getByRole("row"),{key:"F10",shiftKey:true});
  expect(open).toHaveBeenLastCalledWith({tracks:[song],label:"Song"});expect(row).not.toHaveBeenCalled();
});

it("starts Choose playlist on an existing regular playlist and Create on a new one",async()=>{
  vi.mocked(listMusicLibraryPlaylists).mockResolvedValue([{id:2,name:"Regular",description:"",trackCount:1,updatedAt:"rev",editable:true}]);
  const props={onClose:vi.fn(),onSaved:vi.fn()};
  const view=render(<PlaylistAuthoringDialog selection={{tracks:[song],label:"Song",playlistMode:"choose"}} {...props} />);
  await waitFor(()=>expect(screen.getByLabelText("Playlist")).toHaveValue("2"));
  expect(screen.queryByLabelText("Name")).not.toBeInTheDocument();
  view.unmount();
  render(<PlaylistAuthoringDialog selection={{tracks:[song],label:"Song",playlistMode:"create"}} {...props} />);
  await screen.findByRole("option",{name:"Regular"});
  expect(screen.getByLabelText("Playlist")).toHaveValue("");
  expect(screen.getByLabelText("Name")).toBeInTheDocument();
});
