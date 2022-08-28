# Introduction

## ECS

dynec is an ECS (Entity-Component-System) framework,
where game data are mostly stored in *entities* and *components*
and game logic are run in *systems*.

### Game data

An entity refers to a game object,
and a component is a small pieve of data that describes an entity.
I like to visualize them as rows and columns:

| Entity \# | Location | Hitpoint | Experience |
| :---: | :---: | :---: | :---: |
| 0 | (1, 2, 3) | 100 | 5 |
| 1 | (1, 3, 4) | 80 | 4 |
| &vellip; | &vellip; | &vellip; | &vellip; |

Each row is one entity, and each cell is a component of that entity.

Everything can be entities!
For example, in a shooters game, players and bullets may be different entities,
where bullets have different components from players:

| Entity \# | Location | Speed | Damage |
| :---: | :---: | :---: | :---: |
| 0 | (1, 2.5, 3.5) | (0, 0.5, 0.5) | 20 |
| &vellip; | &vellip; | &vellip; | &vellip; |

### Game logic
Game logic is implemented in "systems",
which are basically functions with access to the entity data,
executed once per game tick.
Usually, systems are implemented as loops that execute over all entities of a type
like in the following pseudocode:

```
for each bullet entity {
    location[bullet] += speed[bullet]
}
```
