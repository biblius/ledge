<script>
  import DocumentData from './lib/DocumentData.svelte'
  import SidebarEntry from './lib/SidebarEntry.svelte'
  import showdown from 'showdown'
  import { onMount, setContext } from 'svelte'

  // Handle forward/back buttons
  window.addEventListener("popstate", (event) => {
    // If a state has been provided, we have a "simulated" page
    // and we update the current page.
    if (event.state) {
      // Simulate the loading of the previous page
      displayMain(event.state);
    }
  });

  let id, content, meta;

  const baseUrl = 'http://localhost:3030';

  // Set the document ID to the one in the URL
  const pageUrl = window.location.href;
  const documentId = pageUrl.substring(baseUrl.length + 1);

  async function loadDocumentData(docId) {
    // Prevent loading the same document
    if (id && id === docId) {
      return;
    }

    // Unselect the current doc and select the new one in the sidebar
    selectListItem(docId);

    // Fetch the new document, display popup if not found
    const base = `${baseUrl}/document`;
    const url = docId ? `${base}/${docId}` : base;

    const response = await fetch(url);


    if (response.status === 404) {
      if (docId) {
        // TODO: Popup
        throw new Error(`Not found`);
      }

      // In case of no index, return the default page
      meta = { title: 'Knawledger' };
      content = 'This is my Knawledge.'
    }

    const data = await response.json();

    // Display the contents
    displayMain(data)

    // Push state to history
    if (docId) {
      // FIXME
      history.pushState(data, '', url.replace('3030', '5173').replace('/document', ''));
    }
  };

  /**
   * Set the id, meta and content to the currently selected document
   * @param {{id: string, meta: object, content: string}} documentData
   */
  function displayMain(documentData) {
    const converter = new showdown.Converter(
      {
        ghCodeBlocks: true,
        ghCompatibleHeaderId: true,
      }
    );

    id = documentData.id;
    meta = documentData.meta;
    content = converter.makeHtml(documentData.content);
  }

  async function loadSidebar() {
    const res = await fetch(`${baseUrl}/side`);
    return (await res.json()).filter(el => el.name !== 'index.md');
  }

  function getCurrentMainId() {
    return id;
  }

  /**
   * Unselect the last, then select the currently focused entry in the sidebar
   * @param {string} entryId 
   */
  function selectListItem(entryId) {
    const listItem = document.getElementById(id); 
    if (listItem) {
      listItem.classList.remove('sidebar-selected');
    }

    const newSelected = document.getElementById(entryId);
    if (newSelected) {
      newSelected.classList.add('sidebar-selected');
    }
  }

  setContext('documentMain', { loadDocumentData, getCurrentMainId, selectListItem });

  onMount(async () => {
    loadDocumentData(documentId);
  });
</script>

<nav>
  <h1>
    Knawledger
  </h1>
  {#await loadSidebar()}
    Loading...
  {:then entries}
    <ul>
      {#each entries as entry}
        <SidebarEntry id={entry.id} name={entry.name} title={entry.title} type={entry.type} customId={entry.custom_id}/>
      {/each}
    </ul>
  {/await}
</nav>

<main>
  <DocumentData {content} {meta} />
</main>

<style>
  nav {
    position: sticky;
    left: 0;
    margin: 0 0 1rem 0;
    padding: 0 2rem;
    width: 25%;
  }

  h1 {
    width: 100%;
    height: fit-content;
    margin: 3rem 0;
    font-size: 3.2em;
    text-align: center;
  }

  @media screen and (max-width: 1000px) {
    h1 {
      font-size: 1.6em; 
    }
  }

  ul {
    list-style-type: none;
    padding: 0;
  }

  main {
    margin: 3rem 0;
    padding: 2rem;
    border: 1px solid rgba(255, 255, 255, .1);
    border-radius: 1rem;
    width: 50%;
  }

</style>
