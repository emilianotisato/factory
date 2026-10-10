<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ name: string }>()

function greet(name: string): string {
  return `Hello, ${name}`
}

const useThing = (prefix: string) => {
  return computed(() => `${prefix} ${props.name}`)
}

const label = useThing('Dear')
</script>

<template>
  <p :title="label">{{ greet(props.name) }}</p>
</template>
