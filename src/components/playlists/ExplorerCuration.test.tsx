import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { PlaylistComposer } from "./ExplorerCuration";
import { defaultExplorerFilters } from "../../viewPreferences";

afterEach(()=>{cleanup();vi.restoreAllMocks();Reflect.deleteProperty(HTMLDialogElement.prototype,"showModal");});
it("keeps Album matching when a source query needs an Aurora saved view",()=>{
  Object.defineProperty(HTMLDialogElement.prototype,"showModal",{configurable:true,value:function(this:HTMLDialogElement){this.setAttribute("open","");}});
  render(<PlaylistComposer draft={{kind:"smart",view:"albums",filters:{...defaultExplorerFilters,query:"plays:0"},selection:{kind:"albums",albums:[]}}} onClose={vi.fn()} onSaved={vi.fn()} />);
  expect(screen.getByLabelText("Match",{exact:true})).toHaveValue("albums");
  expect(screen.getByRole("status").textContent).toContain("new shared recipe");
});
