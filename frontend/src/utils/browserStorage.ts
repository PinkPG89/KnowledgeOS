export function getBrowserStorage(): Storage | undefined {
  try {
    return globalThis.localStorage
  } catch {
    // 일부 privacy mode에서는 localStorage property 접근 자체가 예외를 발생시킵니다.
    return undefined
  }
}
