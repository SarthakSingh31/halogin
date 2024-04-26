<script lang="ts">
  import landingPeople from "$lib/landingPeople.png";
</script>

<main class="mx-20">
  <section id="intro-section" class="flex items-center">
    <div>
      <h1 class="text-6xl text-white catamaran-700 mb-2">
        Bringing <span class="creator-text">Creators</span> and
        <span class="brand-text">Brands</span> together
      </h1>
      <span class="text-white hind-vadodara-regular text-xl">
        Our platform connects talented content creators with brands seeking
        powerful sponsorship opportunities. We use cutting-edge machine learning
        to find perfect matches, empowering both sides to make lasting
        connections.
      </span>
    </div>
    <img src={landingPeople} alt="People Meeting" />
  </section>
</main>

<style lang="scss">
  #intro-section {
    img {
      width: calc(50vw - 6rem);
    }
  }

  .creator-text {
    color: #fdd825;
  }

  .brand-text {
    color: #ff64b2;
  }
</style>
