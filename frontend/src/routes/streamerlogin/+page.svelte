<script lang="ts">
    import { GradientButton } from "flowbite-svelte";

 
  </script>
  
  <div class="flex flex-row h-screen">
  
    <div class="text-2xl lg:w-1/2 bg-black flex items-center justify-center p-8 text-white h-full">
      <div class="text-center">
        <h1 class="text-4xl font-extrabold mb-4">Halogin</h1>
        <p class="text-gray-100 mb-4">Work with the best sponsors using Halogin.</p>

        <GradientButton href="/#" outline color="purpleToPink" class="mr-2">Login</GradientButton>
        <p class="text-gray-100">Dont have an account? <a href="/streamerregister">Signup</a></p>
      </div>
    </div>
  
    <div class="lg:w-1/2 lg:flex items-center justify-center p-8 text-white h-full hidden">
      <div class="text-center">
        <h2 class="text-2xl font-semibold mb-4">Additional Content</h2>
        <p class="text-gray-700">Add your additional content here.</p>
        <!-- Add more content as needed -->
      </div>
    </div>
  </div>
  
  <style>
    /* Add your custom styles here or modify existing ones */
    .flex {
      display: flex;
    }
  
    .h-screen {
      height: 100vh;
    }
  
  </style>
  