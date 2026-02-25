import { invoke } from "@tauri-apps/api/core";

interface SpellInfo {
  trigger: string;
  description: string | null;
}

let spells: SpellInfo[] = [];
let selectedIndex = 0;

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

function applySpell(trigger: string) {
  invoke("apply_spell", { trigger });
}

function updateSelection(list: NodeListOf<HTMLLIElement>, index: number) {
  list.forEach((li, i) => li.classList.toggle("selected", i === index));
  list[index]?.scrollIntoView({ block: "nearest" });
}

// Reload spells and reset UI every time the window gets focus
window.addEventListener("focus", () => {
  loadSpells();
  const search = document.getElementById("search") as HTMLInputElement;
  search.value = "";
  search.focus();
});

window.addEventListener("DOMContentLoaded", () => {
  loadSpells();
  const search = document.getElementById("search") as HTMLInputElement;
  search.focus();

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
