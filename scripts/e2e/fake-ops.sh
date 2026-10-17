#!/bin/sh
# A stand-in AI harness for the Ask AI test: replies with ops. A request with
# "twice" gets a reply that does not apply first (a wrong id), then a good one.
# A rethink gets a note and a section; a question an answer with links.
p=$(cat)
case "$p" in
  *"# A question about the board"*) printf 'The API writes to [[db]]; nothing points at [[ghost]].\n' ;;
  *"# Rethink the board"*"Two card products now"*) printf '```json\n{"note":"Scope now has both card products.","ops":[{"op":"in","canvas":"notes","ops":[{"op":"update_node","id":"n1","label":"Two products: virtual cards and spend cards"},{"op":"resolve_comment","id":"c1","resolution":"Scope updated"}]}]}\n```\n' ;;
  *"# Rethink the board"*) printf '```json\n{"note":"Put the storage in its own section.","ops":[{"op":"add_group","id":"storage","label":"Storage","members":["db"]}]}\n```\n' ;;
  *"could not be applied"*) printf '```json\n{"ops":[{"op":"update_node","id":"db","color":"red"}]}\n```\n' ;;
  *twice*) printf '```json\n{"ops":[{"op":"update_node","id":"dbb","color":"red"}]}\n```\n' ;;
  *) printf 'Here you go:\n```json\n{"ops":[{"op":"merge","dsl":"cache: Redis [cylinder]\\napi -> cache: reads"}]}\n```\n' ;;
esac
