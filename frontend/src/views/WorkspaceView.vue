<template>
  <main class="workspace">
    <section class="workspace__intro" aria-labelledby="workspace-title">
      <p class="workspace__eyebrow">B01 · Frontend Foundation</p>
      <h1 id="workspace-title">지식을 사람과 AI가 함께 다루는 작업공간</h1>
      <p class="workspace__description">
        Markdown 파일은 원본으로 유지하고, KnowledgeOS는 모바일과 데스크톱에서 안전하게 탐색하고
        편집하는 인터페이스를 제공합니다.
      </p>
    </section>

    <section class="foundation-grid" aria-label="현재 구현 상태">
      <article>
        <span>완료</span>
        <h2>Rust Filesystem API</h2>
        <p>Vault 경계, Markdown 읽기·쓰기와 Lazy Tree API가 준비됐습니다.</p>
      </article>
      <article>
        <span>현재 단계</span>
        <h2>Vue 3 PWA Shell</h2>
        <p>Router, Pinia, offline shell과 update 보호 기반을 구성합니다.</p>
      </article>
      <article>
        <span>다음 단계</span>
        <h2>Responsive App Shell</h2>
        <p>모바일 drawer와 데스크톱 작업 영역을 분리합니다.</p>
      </article>
    </section>
  </main>
</template>

<style scoped>
.workspace {
  width: min(72rem, 100%);
  margin: 0 auto;
  padding: clamp(2rem, 7vw, 6rem) clamp(1rem, 4vw, 3rem);
}

.workspace__intro {
  max-width: 47rem;
}

.workspace__eyebrow {
  margin: 0 0 1rem;
  color: var(--color-accent);
  font-size: 0.78rem;
  font-weight: 800;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.workspace h1 {
  margin: 0;
  font-size: clamp(2.25rem, 7vw, 5.5rem);
  font-weight: 780;
  letter-spacing: -0.06em;
  line-height: 0.98;
}

.workspace__description {
  max-width: 42rem;
  margin: 1.5rem 0 0;
  color: var(--color-text-muted);
  font-size: clamp(1rem, 2vw, 1.2rem);
  line-height: 1.75;
}

.foundation-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 1rem;
  margin-top: clamp(3rem, 8vw, 7rem);
}

.foundation-grid article {
  min-height: 13rem;
  padding: 1.25rem;
  border: 1px solid var(--color-border);
  border-radius: 1.1rem;
  background: color-mix(in srgb, var(--color-surface) 88%, transparent);
}

.foundation-grid span {
  color: var(--color-accent);
  font-size: 0.75rem;
  font-weight: 800;
}

.foundation-grid h2 {
  margin: 2.5rem 0 0.65rem;
  font-size: 1.1rem;
}

.foundation-grid p {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.9rem;
  line-height: 1.6;
}

@media (max-width: 48rem) {
  .foundation-grid {
    grid-template-columns: 1fr;
  }

  .foundation-grid article {
    min-height: auto;
  }

  .foundation-grid h2 {
    margin-top: 1.25rem;
  }
}
</style>
