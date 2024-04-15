<script lang="ts">
    import { Avatar, Card } from "flowbite-svelte";

    export let data: {
        givenName: string;
        familyName: string;
        pronouns: string;
        pfpImageUrl: string;
        profileDesc: string;
        contentDesc: string;
        audienceDesc: string;
    };
    export let defaults: {
        givenName: string;
        familyName: string;
        pronouns: string;
        profileDesc: string;
        contentDesc: string;
        audienceDesc: string;
    };
    let defaultClass: string;
    export { defaultClass as class };

    $: givenName = data.givenName == "" ? defaults.givenName : data.givenName;
    $: familyName =
        data.familyName == "" ? defaults.familyName : data.familyName;
    $: pronouns = data.pronouns == "" ? defaults.pronouns : data.pronouns;
    $: profileDesc =
        data.profileDesc == "" ? defaults.profileDesc : data.profileDesc;
    $: contentDesc =
        data.contentDesc == "" ? defaults.contentDesc : data.contentDesc;
    $: audienceDesc =
        data.audienceDesc == "" ? defaults.audienceDesc : data.audienceDesc;
</script>

<div class={defaultClass}>
    <Card size="xl">
        <div class="flex mb-2">
            <Avatar size="lg" src={data.pfpImageUrl} border />
            <div class="ml-4">
                <h5
                    class="text-4xl font-bold tracking-tight text-gray-900 dark:text-white"
                >
                    {givenName}
                    {familyName}
                </h5>
                <span class="text-2xl tracking-tight">
                    {pronouns}
                </span>
            </div>
        </div>
        <div class="text-gray-900 dark:text-white">
            <div>
                <h6 class="mt-1 text-md font-bold tracking-tight">Who am I?</h6>
                {profileDesc}
            </div>
            <div>
                <h6 class="mt-1 text-md font-bold tracking-tight">
                    What do I make?
                </h6>
                {contentDesc}
            </div>
            <div>
                <h6 class="mt-1 text-md font-bold tracking-tight">
                    Who watches my content?
                </h6>
                {audienceDesc}
            </div>
        </div>
    </Card>
</div>
