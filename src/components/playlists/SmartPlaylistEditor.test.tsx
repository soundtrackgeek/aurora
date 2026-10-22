import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { saveSmartPlaylist } from "../../playlists";
import { SmartPlaylistEditor } from "./SmartPlaylistEditor";
import { emptySharedRequest } from "../../sharedPlaylistRules";

vi.mock("../../playlists",()=>({saveSmartPlaylist:vi.fn()}));
afterEach(()=>{cleanup();vi.clearAllMocks();});

it("preserves advanced rules and the original edit revision across background refresh",async()=>{
  vi.mocked(saveSmartPlaylist).mockResolvedValue({id:4});
  const props={id:4,name:"Albums",request:{...emptySharedRequest(),view:"albums" as const,filters:{albumRatingMin:85,albumRatingMax:94,billboardRankMax:10,albumArtist:{operator:"equals",value:"Pet Shop Boys"}}},onSaved:vi.fn(),onCancel:vi.fn()};
  const {rerender}=render(<SmartPlaylistEditor {...props} revision="before" />);
  expect(screen.getByLabelText("Minimum rating")).toHaveValue(4.25);
  expect(screen.getByLabelText("Album artist",{exact:true})).toHaveValue("Pet Shop Boys");
  fireEvent.change(screen.getByLabelText("Minimum rating"),{target:{value:"4.5"}});
  fireEvent.change(screen.getByLabelText("Song limit"),{target:{value:"25"}});
  rerender(<SmartPlaylistEditor {...props} revision="after" />);
  fireEvent.click(screen.getByRole("button",{name:"Save Smart playlist"}));
  await waitFor(()=>expect(saveSmartPlaylist).toHaveBeenCalledWith(expect.objectContaining({expectedUpdatedAt:"before",settings:{trackLimit:25,refreshPolicy:"library"},request:expect.objectContaining({filters:{albumRatingMin:90,albumRatingMax:94,billboardRankMax:10,albumArtist:{operator:"equals",value:"Pet Shop Boys"}}})})));
});

it("round-trips seed audio identities and native filters when editing Sonic Smart rules",async()=>{
  vi.mocked(saveSmartPlaylist).mockResolvedValue({id:7});
  const sonic={version:1,profile:"bliss-0.13.0-symphonia-0.6.1-v2-full-mp3",seeds:[{trackKey:"file:seed",title:"Seed song",artist:"Seed artist",audioHash:"original-audio"}],maxDistance:0.15,maxPerArtist:2,ordering:"nearest"};
  render(<SmartPlaylistEditor id={7} revision="original" name="Sonic" request={{...emptySharedRequest(),filters:{yearFrom:1980,billboardRankMax:10}}} settings={{trackLimit:30,refreshPolicy:"manual",sonic}} onSaved={vi.fn()} onCancel={vi.fn()} />);
  expect(screen.getByLabelText("Match",{exact:true})).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Maximum songs per track artist"),{target:{value:"3"}});
  fireEvent.click(screen.getByRole("button",{name:"Save Smart playlist"}));
  await waitFor(()=>expect(saveSmartPlaylist).toHaveBeenCalledWith(expect.objectContaining({expectedUpdatedAt:"original",settings:{trackLimit:30,refreshPolicy:"manual",sonic:{...sonic,maxPerArtist:3}},request:expect.objectContaining({filters:{yearFrom:1980,billboardRankMax:10}})})));
});
