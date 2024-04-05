<script>
  /** @type {any} */
  export let icon = undefined;

  export let route = "";

  export let color = "#166534D9";


  function mouseOver() {
    color = "#FFF";
  }
  function mouseOut() {
    color = "#166534D9";
  }
</script>

<a
  on:mouseover={mouseOver}
  on:focus={mouseOver}
  on:mouseleave={mouseOut}
  href={route}
>
  <svelte:component this={icon} {color} />
</a>
