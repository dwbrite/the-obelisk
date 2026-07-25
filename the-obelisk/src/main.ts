import { invoke } from "@tauri-apps/api/core";

interface Note {
  path: string;
  title: string;
  tags: string[];
  published: boolean;
}

const COLUMNS = ["path", "title", "tags", "published"] as const;

let activeView = "home";
let lastRendered = ""; // JSON of what's currently in the DOM
let tableEl: HTMLTableElement;

function render(notes: Note[]) {
  const header =
      "<tr>" + COLUMNS.map((c) => `<th>${c}</th>`).join("") + "</tr>";
  const rows = notes
      .map(
          (n) =>
              "<tr>" +
              COLUMNS.map((c) => {
                const v = n[c];
                const text = Array.isArray(v) ? v.join(", ") : String(v);
                return `<td>${text}</td>`;
              }).join("") +
              "</tr>",
      )
      .join("");
  tableEl.innerHTML = header + rows;
}

async function tick() {
  try {
    const notes = await invoke<Note[]>("view_notes", { view: activeView });
    const key = activeView + JSON.stringify(notes);
    if (key !== lastRendered) {
      lastRendered = key;
      render(notes);
    }
  } catch (e) {
    console.error("view_notes failed:", e);
  }
  requestAnimationFrame(tick);
}

window.addEventListener("DOMContentLoaded", () => {
  tableEl = document.querySelector("#notes-table")!;
  document.querySelectorAll<HTMLButtonElement>("#view-buttons button").forEach(
      (btn) =>
          btn.addEventListener("click", () => {
            activeView = btn.dataset.view!;
          }),
  );
  requestAnimationFrame(tick);
});
