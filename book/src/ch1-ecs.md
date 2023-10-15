# ECS

Dynec uses concepts from the ECS (Entity-Component-System) paradigm.
It is a data-oriented programming approach that consists of three core concepts:

- An **entity** represent different objects.
- Different **component**s store data for an entity.
- **Systems** process the components to execute game logic.

## Data

An intuitive way to visualize entities and components
would be a table,
where each row is an entity and each cell is a component of that entity:

| Entity \# | Location | Hitpoint | Experience |
| :---: | :---: | :---: | :---: |
| 0 | (1, 2, 3) | 100 | 5 |
| 1 | (1, 3, 4) | 80 | 4 |
| &vellip; | &vellip; | &vellip; | &vellip; |

Everything can be an entity!
For example, in a shooters game,
each player is an entity,
each bullet is an entity,
and even each inventory slot of the player may be an entity as well.

The components for a bullet are different from those for a player:

| Entity \# | Location | Velocity | Damage |
| :---: | :---: | :---: | :---: |
| 0 | (1, 2.5, 3.5) | (0, 0.5, 0.5) | 20 |
| &vellip; | &vellip; | &vellip; | &vellip; |

## Logic

A system is a function that processes the data.
In a typical simulation program,
each system is executed once per "cycle" (a.k.a. "ticks") in a main loop.
Usually, systems are implemented as loops that execute over all entities of a type:

```
for each bullet entity {
    location[bullet] += speed[bullet]
}
```

An ECS framework schedules systems to run on different threads.
Therefore, programs written with ECS are almost lock-free.
