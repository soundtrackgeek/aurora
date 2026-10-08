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
