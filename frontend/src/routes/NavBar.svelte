<script lang="ts">
    import { DarkMode, Heading } from "flowbite-svelte";
    import {
        Navbar,
        NavBrand,
        NavLi,
        NavUl,
        NavHamburger,
        Button,
        Input,
        GradientButton,
    } from "flowbite-svelte";
    import { SearchOutline } from "flowbite-svelte-icons";
    import Logo from "../lib/logo.svg";

    import StreamerLoginComponent from "./StreamerLoginComponent.svelte";
</script>

<Navbar>
    <NavBrand href="/">
        <img src={Logo} class="me-3 h-6 sm:h-9" alt="Halogin Logo" />
        <span
            class="self-center whitespace-nowrap text-xl font-semibold text-primary-600"
        >
            Halogin
        </span>
    </NavBrand>
    <div class="flex md:order-2">
        <StreamerLoginComponent />
        <DarkMode class="border dark:border-gray-800 mr-4 ml-2" />
        <Button
            color="none"
            data-collapse-toggle="mobile-menu-3"
            aria-controls="mobile-menu-3"
            aria-expanded="false"
            class="md:hidden text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-4 focus:ring-gray-200 dark:focus:ring-gray-700 rounded-lg text-sm p-2.5 me-1"
        >
            <!-- <SearchOutline class="w-5 h-5" /> -->
        </Button>
        <div class="hidden relative md:block">
            <div
                class="flex absolute inset-y-0 start-0 items-center ps-3 pointer-events-none"
            >
                <!-- <SearchOutline class="w-4 h-4" /> -->
            </div>
            <!-- <Input id="search-navbar" class="ps-10" placeholder="Search..." /> -->
        </div>
        <NavHamburger />
    </div>
    <NavUl>
        <NavLi href="/">Home</NavLi>
        <NavLi href="/#">About</NavLi>
        <NavLi href="/#">Contact</NavLi>
    </NavUl>

    <div class="flex ml-auto space-x-2 mt-1">
        <GradientButton href="/streamerlogin" outline color="purpleToPink"
            >Login</GradientButton
        >
        <GradientButton
            href="/streamerregister"
            outline
            color="purpleToPink"
            class="mr-2">Signup</GradientButton
        >
    </div>
</Navbar>
