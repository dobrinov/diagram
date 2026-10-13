#!/bin/sh
# A stand-in AI harness for the Ask AI test: replies with ops. A request with
# "twice" gets a reply that does not apply first (a wrong id), then a good one.
p=$(cat)
case "$p" in
  *"could not be applied"*) printf '```json\n{"ops":[{"op":"update_node","id":"db","color":"red"}]}\n```\n' ;;
  *twice*) printf '```json\n{"ops":[{"op":"update_node","id":"dbb","color":"red"}]}\n```\n' ;;
  *) printf 'Here you go:\n```json\n{"ops":[{"op":"merge","dsl":"cache: Redis [cylinder]\\napi -> cache: reads"}]}\n```\n' ;;
esac
