<script>
  import { onMount, getContext } from 'svelte'
  import Icon from './icon/Icon.svelte'
  import MdIcon from './icon/MarkdownIcon.svelte'

  const { loadDocumentData, getCurrentMainId, selectListItem } = getContext('documentMain');

  export let id;
  export let name;
  export let type;
  export let title;
  export let customId = '';
  export let nesting = 0;

  let children = [];
  let loaded = false;

  const baseUrl = 'http://localhost:3030';

  let open = false;

  function toggle(docId) {
    open = !open;
    if (loaded) {
      return;
    }

    loadSideElement(docId);

    loaded = true;
  }

  async function loadSideElement(id) {
    const res = await fetch(`${baseUrl}/side/${id}`);
    children = await res.json();
  }

  onMount(() => {
    if (id === getCurrentMainId()) {
      selectListItem(id);
    }
  });
</script>

<li 
  {id}
  style="margin-left: {nesting}rem;"
  on:click={() => type === 'd' ? toggle(id) : loadDocumentData(customId ? customId : id)}
>
  <p>
    {#if name.endsWith('.md')}
      <Icon icon={MdIcon} />
    {/if}
    {title ? title : name}
  </p>
</li>

{#if open}
  {#each children as child}
      <svelte:self title={child.title} id={child.id} name={child.name} type={child.type} customId={child.customId} nesting={nesting + 0.75}/>
  {/each}
{/if}

<style>
  li {
    border: 1px solid transparent;
    box-sizing: border-box;
  }

  p {
    box-sizing: border-box;
    position: relative;
    text-wrap: wrap;
    padding: 0.5rem 1rem;
    padding-left: 2rem;
    font-size: 1.3em;
  }

  p:hover {
    cursor: pointer;
  }
</style>
