"""Artifact providers: turn a missing input key into the job spec that produces it.

A key is `<provider>:...`; the recipe was recorded when a kind asked for the key
(ctx.binary). Adding a provider is one function here or a module-level PROVIDERS dict
in a kind module.
"""

import importlib
import pkgutil

from .core import SpecError


def binary(key, recipe, ctx):
    spec = dict(kind="build", name=f"build-{recipe['name']}-{recipe['commit'][:8]}-{recipe['arch']}",
                repo=recipe["repo"], commit=recipe["commit"], arch=recipe["arch"], target=recipe["name"],
                dirty=recipe.get("dirty", False), features=recipe.get("features", []), priority=10)
    for field in ("package", "example", "bin"):
        if recipe.get(field):
            spec[field] = recipe[field]
    return spec


PROVIDERS = {"binary": binary}


def spec_for(key, recipe, ctx):
    from . import kinds
    table = dict(PROVIDERS)
    for info in pkgutil.iter_modules(kinds.__path__):
        table.update(getattr(importlib.import_module(f"{kinds.__name__}.{info.name}"), "PROVIDERS", {}))
    provider = table.get(recipe.get("provider") or key.split(":", 1)[0])
    if provider is None:
        raise SpecError(f"no provider for {key}")
    return provider(key, recipe, ctx)
