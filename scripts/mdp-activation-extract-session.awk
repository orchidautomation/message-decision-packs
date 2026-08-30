#!/usr/bin/env awk -f
# Extract a session id from a JSON hook payload.
#
# Searches documented keys in priority order and prints the first string
# value found. Returns nothing for payloads missing all keys or that
# contain malformed JSON.
#
# Keys (priority order):
#   session_id, sessionId, conversationId, conversation_id,
#   session_id_v2, session_ulid

# Quote (34) and colon (58). Build "key\":" so awk matches the visible
# JSON prefix even when the source file already contains single quotes.
BEGIN {
  Q = sprintf("%c", 34)
  C = sprintf("%c", 58)
  split("session_id sessionId conversationId conversation_id session_id_v2 session_ulid", fields, " ")
  nf = length(fields)
}
{
  for (f = 1; f <= nf; f++) {
    key = fields[f]
    # Match "key": then optional whitespace then opening quote.
    key_marker = Q key Q C
    p = index($0, key_marker)
    if (p <= 0) continue
    rest = substr($0, p + length(key_marker))
    while (length(rest) > 0 && (substr(rest, 1, 1) == " " || substr(rest, 1, 1) == "\t")) {
      rest = substr(rest, 2)
    }
    if (length(rest) <= 0 || substr(rest, 1, 1) != Q) continue
    rest = substr(rest, 2)
    q = index(rest, Q)
    if (q <= 0) continue
    value = substr(rest, 1, q - 1)
    if (length(value) > 0 && length(value) <= 256) {
      print value
      exit
    }
  }
}