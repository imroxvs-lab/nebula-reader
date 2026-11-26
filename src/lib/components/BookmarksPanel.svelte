<script lang="ts">
  import { formatPercent } from "#lib/format.ts";
  import type { BookmarkList } from "#lib/stores/bookmarks.svelte.ts";
  import type { Bookmark } from "#lib/types.ts";

  let {
    list,
    marked,
    ontoggle,
    onjump,
  }: {
    list: BookmarkList;
    /** Si la posición actual ya tiene marcador. */
    marked: boolean;
    ontoggle: () => void;
    onjump: (bookmark: Bookmark) => void;
  } = $props();
</script>

<div class="bookmarks">
  <button class="toggle" onclick={ontoggle}>
    <span class="icon" aria-hidden="true">{marked ? "\uE8D9" : "\uE734"}</span>
    {marked ? "Quitar marcador de aquí" : "Añadir marcador aquí"}
    <kbd>Ctrl+D</kbd>
  </button>
  {#if list.items.length === 0}
    <p class="empty">Todavía no hay marcadores en este libro.</p>
  {:else}
    <ul aria-label="Marcadores">
      {#each list.items as bookmark (bookmark.id)}
        <li>
          <button class="jump" onclick={() => onjump(bookmark)}>
            <span class="label">{bookmark.label}</span>
            <span class="percent">{formatPercent(bookmark.percent)}</span>
          </button>
          <button
            class="delete"
            aria-label="Quitar el marcador {bookmark.label}"
            title="Quitar"
            onclick={() => void list.remove(bookmark)}
          >
            <span class="icon" aria-hidden="true">&#xE711;</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .bookmarks {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  button {
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: inherit;
  }

  button:hover {
    background: var(--panel-hover, rgba(127, 127, 127, 0.14));
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 36px;
    padding: 0 10px;
    border: 1px solid var(--panel-line, rgba(127, 127, 127, 0.3));
    text-align: left;
  }

  .toggle kbd {
    margin-left: auto;
    font-family: inherit;
    font-size: 11px;
    opacity: 0.7;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: flex;
    align-items: center;
  }

  .jump {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 8px;
    padding: 7px 10px;
    text-align: left;
  }

  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .percent {
    font-size: 12px;
    opacity: 0.7;
    font-variant-numeric: tabular-nums;
  }

  .delete {
    width: 30px;
    height: 30px;
    font-size: 10px;
  }

  .empty {
    margin: 4px 2px;
    font-size: 12px;
    opacity: 0.75;
  }
</style>
