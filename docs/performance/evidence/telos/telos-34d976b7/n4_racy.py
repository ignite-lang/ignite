#!/usr/bin/env python3
import sys

path = sys.argv[1]
src = open(path, encoding="utf-8").read()
old = """    let changed = todos.update(id, |todo| Todo {
      title = value.text("title").unwrap_or(todo.title)
      done = value.bool("done").unwrap_or(todo.done)
      ..todo
    })
"""
new = """    let changed = match todos.get(id) {
      Some(todo) => {
        let next = Todo {
          title = value.text("title").unwrap_or(todo.title)
          done = value.bool("done").unwrap_or(todo.done)
          ..todo
        }
        todos.put(id, next)
        Some(next)
      }
      None => None
    }
"""
assert src.count(old) == 1
open(path, "w", encoding="utf-8").write(src.replace(old, new))
