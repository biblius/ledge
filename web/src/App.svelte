<script>
  import DocumentData from "./lib/DocumentData.svelte";
  import SidebarEntry from "./lib/SidebarEntry.svelte";
  import showdown from "showdown";
  import { onMount, setContext } from "svelte";
  const baseUrl = import.meta.env.VITE_BASE_URL;

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

  // Set the document ID to the one in the URL
  const pageUrl = window.location.href;
  const documentId = pageUrl.substring(baseUrl.length + 1);

  /**
   * Fetch a document from the backend and display it on the page.
   * @param {?string} docId The UUID of the document
   * @param {?string} customId The user specified custom ID used to display nicer URLs.
   */
  async function loadDocumentData(docId, customId) {
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
      meta = { title: "Ledgeknaw" };
      content = "This is my knawledge.";
    }

    const data = await response.json();

    // Display the contents
    displayMain(data);

    // Push state to history
    if (docId) {
      let historyUrl = url.replace("/document", "");
      if (customId) {
        historyUrl = historyUrl.replace(docId, customId);
      }
      history.pushState(data, "", historyUrl);
    }
  }

  /**
   * Set the id, meta and content to the currently selected document
   * @param {{id: string, meta: object, content: string}} documentData
   */
  function displayMain(documentData) {
    const converter = new showdown.Converter({
      ghCodeBlocks: true,
      ghCompatibleHeaderId: true,
    });

    id = documentData.id;
    meta = documentData.meta;
    content = converter.makeHtml(documentData.content);
  }

  async function loadSidebar() {
    const res = await fetch(`${baseUrl}/side`);
    return (await res.json()).filter((el) => el.name !== "index.md");
  }

  function getCurrentMainId() {
    return id;
  }

  /**
   * Unselect the last, then select the currently focused entry in the sidebar
   * @param {string} entryId
   */
  function selectListItem(entryId) {
    const listItem = document.getElementById(`side_${id}`);
    if (listItem) {
      listItem.classList.remove("sidebar-selected");
    }

    const newSelected = document.getElementById(`side_${entryId}`);
    if (newSelected) {
      newSelected.classList.add("sidebar-selected");
    }
  }

  setContext("documentMain", {
    loadDocumentData,
    getCurrentMainId,
    selectListItem,
  });

  onMount(async () => {
    loadDocumentData(documentId, null);
  });
</script>

<nav>
  <h1>
    <a href="/"> Ledgeknaw </a>
  </h1>
  {#await loadSidebar()}
    Loading...
  {:then entries}
    <ul>
      {#each entries as entry}
        <SidebarEntry
          id={entry.id}
          name={entry.name}
          title={entry.title}
          type={entry.type}
          custom_id={entry.custom_id}
        />
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
    top: 0;
    margin: 0 0 1rem 0;
    padding: 0 2rem;
    width: 25%;
    height: 100%;
  }

  h1 {
    width: 100%;
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
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 1rem;
    width: 50%;
  }
</style>
