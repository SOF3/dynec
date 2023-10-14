# ECS

Dynec uses concepts from the ECS (Entity-Component-System) paradigm.
It is a data-oriented programming approach that consists of three core concepts:

- 
data are mostly stored in "components" for different "entities",
and logic is run in "systems" that process the data.

## Data

An entity corresponds to an object.
Different components store the data related to an object.
I like visualizing them as rows and columns,
where each row is an entity and each cell is a component of that entity:

| Entity \# | Location | Hitpoint | Experience |
| :---: | :---: | :---: | :---: |
| 0 | (1, 2, 3) | 100 | 5 |
| 1 | (1, 3, 4) | 80 | 4 |
| &vellip; | &vellip; | &vellip; | &vellip; |

Everything can be entities!
For example, in a shooters game,
each player and and each bullet is a separate entity.
The components for a bullet are different from those for a player:

| Entity \# | Location | Speed | Damage |
| :---: | :---: | :---: | :---: |
| 0 | (1, 2.5, 3.5) | (0, 0.5, 0.5) | 20 |
| &vellip; | &vellip; | &vellip; | &vellip; |

Unlike the traitional OOP pattern where
components of the same object are stored together,
ECS typically stores components of the same type together.
Since data of the same type are usually processed together in bulk,
CPU cache lines have much better efficiency
compared to the traditional random access on the heap.

## Logic

A system is a function that processes data.
In a typical game or simulation,
each system are executed once per "cycle" (a.k.a. "ticks") in the main loop.
Usually, systems are implemented as loops that execute over all entities of a type:

```
for each bullet entity {
    location[bullet] += speed[bullet]
}
```

An ECS framework schedules systems that can be run together on different threads.
Therefore, programs written with ECS are almost lock-free,
so they are more efficient on a multi-threaded environment
compared to traditional approaches that might result in frequent lock contention.
