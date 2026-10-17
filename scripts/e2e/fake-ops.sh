#!/bin/sh
# A stand-in AI harness for the Ask AI test: replies with ops. A request with
# "twice" gets a reply that does not apply first (a wrong id), then a good one.
# A rethink gets a note and a section.
p=$(cat)
case "$p" in
  *"# Rethink the board"*) printf '```json\n{"note":"Put the storage in its own section.","ops":[{"op":"add_group","id":"storage","label":"Storage","members":["db"]}]}\n```\n' ;;
  *"could not be applied"*) printf '```json\n{"ops":[{"op":"update_node","id":"db","color":"red"}]}\n```\n' ;;
  *twice*) printf '```json\n{"ops":[{"op":"update_node","id":"dbb","color":"red"}]}\n```\n' ;;
  *) printf 'Here you go:\n```json\n{"ops":[{"op":"merge","dsl":"cache: Redis [cylinder]\\napi -> cache: reads"}]}\n```\n' ;;
esac
