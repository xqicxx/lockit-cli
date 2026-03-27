<script>
  import { invoke } from '@tauri-apps/api/core'
  
  let greeting = $state('Loading...')
  
  async function hello() {
    try {
      greeting = await invoke('greet', { name: 'Lockit' })
    } catch (e) {
      greeting = 'Hello from Lockit Mobile!'
    }
  }
  
  hello()
</script>

<main class="flex flex-col items-center justify-center min-h-screen bg-gray-900 text-white">
  <h1 class="text-4xl font-bold mb-4">🔐 Lockit</h1>
  <p class="text-xl text-gray-300">{greeting}</p>
  <button 
    onclick={hello}
    class="mt-6 px-6 py-3 bg-blue-600 rounded-lg hover:bg-blue-700 transition"
  >
    Refresh
  </button>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  }
</style>
