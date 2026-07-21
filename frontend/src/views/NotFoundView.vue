<template>
  <main class="not-found">
    <p>404</p>
    <h1>요청한 화면을 찾을 수 없습니다.</h1>
    <RouterLink to="/">KnowledgeOS로 돌아가기</RouterLink>
  </main>
</template>

<style scoped>
.not-found {
  display: grid;
  min-height: 100dvh;
  place-content: center;
  padding: 1rem;
  text-align: center;
}

.not-found p {
  margin: 0;
  color: var(--color-accent);
  font-size: 0.85rem;
  font-weight: 800;
}

.not-found h1 {
  margin: 0.75rem 0 1.5rem;
  font-size: clamp(1.75rem, 5vw, 3rem);
}

.not-found a {
  color: var(--color-text);
  font-weight: 700;
}
</style>
