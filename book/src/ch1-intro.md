# ECS

dynec is an ECS (Entity-Component-System) framework.
Data are mostly stored in "components" for different "entities".
Logic is run in "systems" that process the data.

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
