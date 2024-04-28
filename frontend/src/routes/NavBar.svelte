<script>
    import { Navbar, NavBrand, Checkbox, Modal } from "flowbite-svelte";
    import { SearchOutline } from "flowbite-svelte-icons";
    import GoogleLogin from "./GoogleLogin.svelte";
    import TwitchLogin from "./TwitchLogin.svelte";

    let open = false;
    let keepLoggedIn = false;

    let dropdown = "creators";

    $: searchPlaceholder =
        dropdown === "creators" ? "Find creators" : "Find brand deals";
</script>

<Navbar class="dark:bg-black">
    <NavBrand href="/">
        <span
            class="self-center whitespace-nowrap text-3xl encode-sans-expanded-black text-white"
        >
            mercant
        </span>
    </NavBrand>
    <div class="flex">
        <form
            id="nav-search"
            class="border-2 border-white rounded-md p-2 flex items-center mr-4"
            action="/search"
        >
            <button class="inline-flex">
                <SearchOutline class="inline mr-2 text-white" />
            </button>
            <input
                name="searchQuery"
                class="text-white"
                placeholder={searchPlaceholder}
            />
            <span class="bg-gray-600 w-0.5 h-6 inline-block mx-2" />
            <select
                bind:value={dropdown}
                name="searchGroup"
                class="text-md text-center text-white"
            >
                <option value="creators">Creators</option>
                <option value="brandDeals">Brand Deals</option>
            </select>
        </form>
        <button
            class="border-2 border-white rounded-md p-2 hover:bg-gray-800 text-white"
            on:click={() => (open = true)}
        >
            Sign In / Sign Up
        </button>
    </div>
</Navbar>
<Modal bind:open>
    <GoogleLogin
        bind:keepLoggedIn
        onSuccess={() => (window.location.pathname = "creator/profile")}
    />
    <TwitchLogin
        bind:keepLoggedIn
        onSuccess={() => (window.location.pathname = "creator/profile")}
    />
    <Checkbox bind:checked={keepLoggedIn}>Keep logged in</Checkbox>
</Modal>

<style lang="scss">
    #nav-search {
        input {
            width: 300px;
            background: transparent;

            &:placeholder {
                color: #fff;
            }

            &:focus-visible {
                outline: 0;
            }
        }

        select {
            border: 0;
            padding: 0 15px 0 0;
            outline: 0;
            background-position: right 0rem center;
            background-color: transparent;
            background-image: url("data:image/svg+xml,%3Csvg%20aria-hidden%3D%27true%27%20xmlns%3D%27http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%27%20fill%3D%27none%27%20viewBox%3D%270%200%2010%206%27%3E%20%3Cpath%20stroke%3D%27%23ffffff%27%20stroke-linecap%3D%27round%27%20stroke-linejoin%3D%27round%27%20stroke-width%3D%272%27%20d%3D%27m1%201%204%204%204-4%27%2F%3E%20%3C%2Fsvg%3E");
        }
    }
</style>
