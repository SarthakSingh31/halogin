<script lang="ts">
    import { onMount } from "svelte";

    onMount(() => {
        let params = window.location.search
            .substring(1)
            .split("&")
            .map((query) => query.split("="))
            .reduce((acc: Map<string, string>, val) => {
                acc.set(val[0], val[1]);
                return acc;
            }, new Map());
        window.opener.twitchCallback(params.get("state"), params.get("code"));
        window.close();
    });
</script>
