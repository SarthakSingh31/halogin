<script lang="ts">
    import { onMount } from "svelte";
    import "../app.pcss";
    import { wsRpc } from "$lib/ws";

    onMount(() => {
        wsRpc.connect();
    });
</script>

<slot />
