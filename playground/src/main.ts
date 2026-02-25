import { invoke } from "@tauri-apps/api/core";

document.querySelector("#apply-btn")?.addEventListener("click", () => {
  invoke("apply_spell");
});
