import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface SpellInfo {
  trigger: string;
  description: string | null;
}

type SpellResult =
  | { mode: "done" }
  | { mode: "preview"; content: string }
  | { mode: "stream" };

let spells: SpellInfo[] = [];
let selectedIndex = 0;

// ---- State management ----

function showSelector() {
  document.getElementById("selector")!.style.display = "flex";
  document.getElementById("preview")!.style.display = "none";
  const search = document.getElementById("search") as HTMLInputElement;
  search.focus();
}

function showPreview(streaming: boolean) {
  document.getElementById("selector")!.style.display = "none";
  const preview = document.getElementById("preview")!;
  preview.style.display = "flex";
  document.getElementById("preview-label")!.textContent = streaming
    ? "Output (streaming…)"
    : "Output";
}

// ---- Spell list ----

async function loadSpells() {
  spells = await invoke<SpellInfo[]>("get_spells");
  renderSpells(spells);
}

function renderSpells(list: SpellInfo[]) {
  const ul = document.getElementById("spell-list")!;
  const empty = document.getElementById("empty")!;
  ul.innerHTML = "";
  selectedIndex = 0;

  if (list.length === 0) {
    empty.style.display = "block";
    return;
  }
  empty.style.display = "none";

  list.forEach((spell, i) => {
    const li = document.createElement("li");
    if (i === 0) li.classList.add("selected");

    const trigger = document.createElement("span");
    trigger.className = "trigger";
    trigger.textContent = spell.trigger;
    li.appendChild(trigger);

    if (spell.description) {
      const desc = document.createElement("span");
      desc.className = "desc";
      desc.textContent = spell.description;
      li.appendChild(desc);
    }

    li.addEventListener("click", () => applySpell(spell.trigger));
    ul.appendChild(li);
  });
}

function updateSelection(list: NodeListOf<HTMLLIElement>, index: number) {
  list.forEach((li, i) => li.classList.toggle("selected", i === index));
  list[index]?.scrollIntoView({ block: "nearest" });
}

async function applySpell(trigger: string) {
  const result = await invoke<SpellResult>("apply_spell", { trigger });
  if (result.mode === "preview") {
    document.getElementById("preview-content")!.textContent = result.content;
    showPreview(false);
  } else if (result.mode === "stream") {
    document.getElementById("preview-content")!.textContent = "";
    showPreview(true);
  }
  // mode === "done": window is already hiding/hidden, nothing to do
}

// ---- Window focus: reset to selector ----

window.addEventListener("focus", () => {
  showSelector();
  loadSpells();
  const search = document.getElementById("search") as HTMLInputElement;
  search.value = "";
  search.focus();
});

// ---- Init ----

window.addEventListener("DOMContentLoaded", async () => {
  await listen<string>("spell-stream", (event) => {
    const content = document.getElementById("preview-content")!;
    content.textContent += event.payload;
    content.scrollTop = content.scrollHeight;
  });

  await listen<null>("spell-stream-end", () => {
    document.getElementById("preview-label")!.textContent = "Output";
  });

  loadSpells();
  const search = document.getElementById("search") as HTMLInputElement;
  search.focus();

  document.getElementById("preview-close")!.addEventListener("click", () => {
    invoke("cancel");
  });

  search.addEventListener("input", () => {
    const query = search.value.toLowerCase();
    const filtered = spells.filter(
      (s) =>
        s.trigger.toLowerCase().includes(query) ||
        (s.description?.toLowerCase().includes(query) ?? false)
    );
    renderSpells(filtered);
  });

  document.addEventListener("keydown", (e) => {
    const previewVisible =
      document.getElementById("preview")!.style.display !== "none";

    if (previewVisible) {
      if (e.key === "Escape") invoke("cancel");
      return;
    }

    const items = document.querySelectorAll<HTMLLIElement>("#spell-list li");
    if (items.length === 0) return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, items.length - 1);
      updateSelection(items, selectedIndex);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
      updateSelection(items, selectedIndex);
    } else if (e.key === "Enter") {
      items[selectedIndex]?.click();
    } else if (e.key === "Escape") {
      invoke("cancel");
    }
  });
});
