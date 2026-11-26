<script lang="ts" module>
  export interface MenuItem {
    label: string;
    glyph?: string;
    danger?: boolean;
    /** Casilla marcada (para opciones de alternar). */
    checked?: boolean;
    disabled?: boolean;
    action?: () => void;
    submenu?: MenuItem[];
    /** Separador antes de este elemento. */
    separated?: boolean;
  }
</script>

<script lang="ts">
  import { tick } from "svelte";

  let {
    x,
    y,
    items,
    label,
    onclose,
  }: { x: number; y: number; items: MenuItem[]; label: string; onclose: () => void } = $props();

  const WIDTH = 248;
  const ITEM_HEIGHT = 34;

  let root: HTMLDivElement | undefined = $state();
  let openSub = $state<number | null>(null);
  let subRoot: HTMLDivElement | undefined = $state();
  /** Elemento con el foco antes de abrir el menú, para devolvérselo al cerrar. */
  const previousFocus = document.activeElement as HTMLElement | null;

  const position = $derived({
    left: Math.max(8, Math.min(x, window.innerWidth - WIDTH - 8)),
    top: Math.max(8, Math.min(y, window.innerHeight - items.length * ITEM_HEIGHT - 16)),
  });
  const subOnLeft = $derived(position.left + WIDTH * 2 + 8 > window.innerWidth);

  $effect(() => {
    void tick().then(() => focusables(root)[0]?.focus());
  });

  function focusables(container: HTMLElement | undefined): HTMLButtonElement[] {
    return container ? [...container.querySelectorAll<HTMLButtonElement>(":scope > [role^='menuitem']:not(:disabled)")] : [];
  }

  function close(restoreFocus = true) {
    onclose();
    if (restoreFocus) previousFocus?.focus?.();
  }

  function activate(item: MenuItem, index: number) {
    if (item.submenu) {
      void openSubmenu(index);
      return;
    }
    close();
    item.action?.();
  }

  async function openSubmenu(index: number) {
    openSub = index;
    await tick();
    focusables(subRoot)[0]?.focus();
  }

  function move(container: HTMLElement | undefined, delta: number) {
    const list = focusables(container);
    if (list.length === 0) return;
    const current = list.indexOf(document.activeElement as HTMLButtonElement);
    list[(current + delta + list.length) % list.length]?.focus();
  }

  function onKeydown(event: KeyboardEvent, inSub: boolean) {
    const container = inSub ? subRoot : root;
    switch (event.key) {
      case "ArrowDown":
        move(container, 1);
        break;
      case "ArrowUp":
        move(container, -1);
        break;
      case "Home":
        focusables(container)[0]?.focus();
        break;
      case "End":
        focusables(container).at(-1)?.focus();
        break;
      case "ArrowRight": {
        if (inSub) return;
        const index = Number((document.activeElement as HTMLElement)?.dataset.index);
        if (items[index]?.submenu) void openSubmenu(index);
        break;
      }
      case "ArrowLeft":
      case "Escape":
        if (inSub) {
          const index = openSub;
          openSub = null;
          void tick().then(() => focusables(root)[index ?? 0]?.focus());
        } else close();
        break;
      case "Tab":
        close();
        return;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
  }

  function onWindowPointerDown(event: PointerEvent) {
    const target = event.target as Node;
    if (!root?.contains(target) && !subRoot?.contains(target)) close(false);
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} onblur={() => close(false)} onresize={() => close(false)} />

{#snippet entry(item: MenuItem, index: number, inSub: boolean)}
  {#if item.separated}<div class="separator" role="separator"></div>{/if}
  <button
    role={item.checked === undefined ? "menuitem" : "menuitemcheckbox"}
    aria-checked={item.checked === undefined ? undefined : item.checked}
    aria-haspopup={item.submenu ? "menu" : undefined}
    aria-expanded={item.submenu ? openSub === index : undefined}
    data-index={index}
    class:danger={item.danger}
    disabled={item.disabled}
    tabindex="-1"
    onclick={() => activate(item, index)}
    onpointerenter={() => {
      if (!inSub) openSub = item.submenu ? index : null;
    }}
  >
    <span class="icon glyph" aria-hidden="true">{item.checked ? "\uE73E" : (item.glyph ?? "")}</span>
    <span class="text">{item.label}</span>
    {#if item.submenu}<span class="icon chevron" aria-hidden="true">&#xE76C;</span>{/if}
  </button>
{/snippet}

<div
  class="menu"
  role="menu"
  tabindex="-1"
  aria-label={label}
  bind:this={root}
  style:left="{position.left}px"
  style:top="{position.top}px"
  onkeydown={(e) => onKeydown(e, false)}
>
  {#each items as item, index (index)}
    {@render entry(item, index, false)}
    {#if openSub === index && item.submenu}
      <div
        class="menu sub"
        role="menu"
        tabindex="-1"
        aria-label={item.label}
        bind:this={subRoot}
        style:left="{subOnLeft ? position.left - WIDTH + 4 : position.left + WIDTH - 4}px"
        style:top="{position.top + index * ITEM_HEIGHT}px"
        onkeydown={(e) => onKeydown(e, true)}
      >
        {#each item.submenu as subItem, subIndex (subIndex)}
          {@render entry(subItem, subIndex, true)}
        {/each}
      </div>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 30;
    width: 248px;
    max-height: calc(100vh - 16px);
    overflow-y: auto;
    padding: 4px;
    border-radius: var(--radius-md);
    border: 1px solid var(--card-stroke);
    background: var(--flyout);
    box-shadow: var(--shadow);
    animation: appear 120ms var(--ease);
  }

  .menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 34px;
    padding: 0 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
  }

  .menu button:hover,
  .menu button:focus-visible,
  .menu button[aria-expanded="true"] {
    background: var(--subtle-hover);
    outline: none;
  }

  .menu button:focus-visible {
    box-shadow: inset 0 0 0 2px var(--accent);
  }

  .menu button:disabled {
    opacity: 0.45;
  }

  .danger {
    color: var(--danger);
  }

  .glyph {
    width: 16px;
    font-size: 13px;
  }

  .text {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chevron {
    font-size: 10px;
  }

  .separator {
    height: 1px;
    margin: 4px 6px;
    background: var(--card-stroke);
  }

  @keyframes appear {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
</style>
