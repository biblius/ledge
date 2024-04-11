<script>
  import { onMount, getContext } from 'svelte'
  import Icon from './icon/Icon.svelte'
  import MdIcon from './icon/MarkdownIcon.svelte'
  import DirIcon from './icon/DirectoryIcon.svelte'

  const { loadDocumentData, getCurrentMainId, selectListItem } = getContext('documentMain');

  export let id;
  export let name;
  export let type;
  export let title;
  export let custom_id;
  export let nesting = 0;

  let children = [];
  let loaded = false;

  const baseUrl = import.meta.env.VITE_BASE_URL;

  let open = false;

  /**
   * Open/close a sidebar directory entry
   * @param docId {string}
   */
  function toggle(docId) {
    open = !open;

    if (loaded) {
      return;
    }

    loadSideElement(docId);

    loaded = true;
  }

  /**
   * Fetch and append the children of the target directory
   * @param {string} id
   */
  async function loadSideElement(id) {
    const res = await fetch(`${baseUrl}/side/${id}`);
    const data = await res.json();
    children = data;
  }

  onMount(() => {
    // Always load the root directory elements to save the extra click
    if (nesting === 0) {
     toggle(id);
    }

    if (id === getCurrentMainId()) {
      selectListItem(id);
    }
  });
</script>

<li 
  style="margin-left: {nesting}rem;"
  on:click={() => type === 'd' ? toggle(id) : loadDocumentData(id, custom_id)}
>
  {#if nesting !== 0}
    <svg width="10" height="30" xmlns="http://www.w3.org/2000/svg">
      <line x1="0" y1="20" x2="100" y2="20" stroke="white" stroke-width="2"/>
      <line x1="0" y1="0" x2="0" y2="20" stroke="white" stroke-width="2"/>
    </svg>
  {/if}

  <p id={`side_${id}`} >
    {#if name.endsWith('.md')}
      <Icon icon={MdIcon} />
    {:else}
      <Icon icon={DirIcon} />
    {/if}
    {title ? title : name}
  </p>
</li>

{#if open}
  {#each children as child}
    <svelte:self 
      title={child.title} 
      id={child.id}
      name={child.name}
      type={child.type}
      customId={child.customId}
      nesting={nesting + 1.2}
    />
  {/each}
{/if}

<style>
  li {
    position: relative;
  }

  svg {
    position: absolute;
    opacity: 0.2;
    left: 1rem;
  }

  p {
    box-sizing: border-box;
    position: relative;
    text-wrap: wrap;
    padding-left: 2rem;
    font-size: 1.3em;
  }

  p:hover {
    cursor: pointer;
  }
</style>
