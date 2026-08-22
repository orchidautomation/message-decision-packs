// Keep host adapters on one bounded recommendation. The Rust run kernel is
// still the authority; these values only validate the transport guard.
export const RECOMMENDED_TIMEOUT_MS = 60_000
export const MAX_TIMEOUT_MS = 300_000
export const FINALIZATION_RESERVE_MS = 250
export const MIN_TIMEOUT_MS = FINALIZATION_RESERVE_MS + 1

export const validateTransportTimeout = (value) => {
  if (!Number.isSafeInteger(value) || value < MIN_TIMEOUT_MS || value > MAX_TIMEOUT_MS) {
    throw new Error(`timeout_ms must be an integer between ${MIN_TIMEOUT_MS} and ${MAX_TIMEOUT_MS}`)
  }
  return value
}

export const deadlineWarning = (runtimeMs, transportMs) => {
  if (transportMs > runtimeMs) return 'outer-timeout-cannot-extend-inner'
  if (transportMs - FINALIZATION_RESERVE_MS < runtimeMs) return 'outer-timeout-truncates-runtime'
  return null
}
