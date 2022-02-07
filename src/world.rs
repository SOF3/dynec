//! The root storage for all entities of all archetypes.

use std::any::{self, Any, TypeId};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops;
use std::sync::{Arc, RwLock, RwLockWriteGuard};

use bitvec::prelude::BitVec;
use itertools::Itertools;
use xias::Xias;

use crate::optvec::OptVec;
use crate::storage::{AnyColumn, Column, ColumnStorage};
use crate::{archetype, component, entity, system, Archetype, Component, Entity, System};

/// The root storage of all entities and components.
#[derive(Default)]
pub struct World {
    tables: BTreeMap<TypeId, Arc<dyn Any>>,
}

impl World {
    /// Schedules a system.
    pub fn schedule<S: System>(&mut self, system: S) {
        let id = TypeId::of::<S>();
        let system = Box::new(system) as Box<dyn system::AnySystem>;
    }

    fn get_table<A: Archetype>(&self) -> &Table<A> {
        let table = match self.tables.get(&TypeId::of::<A>()) {
            Some(table) => table,
            None => panic!("Unregistered archetype {:?}", TypeId::of::<A>()),
        };
        table.downcast_ref::<Table<A>>().expect("TypeId mismatch")
    }

    fn get_table_mut<A: Archetype>(&mut self) -> &mut Table<A> {
        let table = match self.tables.get_mut(&TypeId::of::<A>()) {
            Some(table) => table,
            None => panic!("Unused archetype {:?}", any::type_name::<A>()),
        };
        let table = Arc::get_mut(table).expect("Arc<Table> is leaked");
        table.downcast_mut::<Table<A>>().expect("TypeId mismatch")
    }

    fn get_table_mut_or_init<A: Archetype>(&mut self) -> &mut Table<A> {
        let table = self.tables.entry(TypeId::of::<A>()).or_insert_with(|| {
            Arc::new(Table::<A> {
                entities: entity::Counter::default(),
                columns: BTreeMap::new(),
                multi_ctors: BTreeMap::new(),
                _ph: PhantomData,
            })
        });
        let table = Arc::get_mut(table).expect("Arc<Table> is leaked");
        table.downcast_mut::<Table<A>>().expect("TypeId mismatch")
    }

    /// Creates a new entity.
    pub fn create<A: Archetype>(
        &mut self,
        near: Option<entity::Weak<A>>,
        initials: component::Initials<A>,
    ) -> Entity<A> {
        self.get_table_mut::<A>().insert(near, initials)
    }

    /// Removes an entity.
    pub fn remove<A: Archetype>(&mut self, entity: Entity<A>) {
        self.get_table_mut::<A>().remove(entity)
    }

    /// Gets a specific single-component.
    ///
    /// This method should only be used for debugging and testing.
    pub fn get<A: Archetype, C: component::Single, R, F: FnOnce(&mut C) -> R>(
        &mut self,
        entity: Entity<A>,
        f: F,
    ) -> Option<R>
    where
        A: archetype::Contains<C>,
    {
        let storage = &self.get_table_mut::<A>().single_column_mut::<C>().storage;
        let mut storage = storage.write().expect("Another thread panicked");
        Some(f(storage.get_mut(entity.id)?))
    }

    /// Gets a specific multi-component.
    ///
    /// This method should only be used for debugging and testing.
    pub fn get_multi<A: Archetype, C: component::Multi, F: FnOnce(&mut C) -> R, R>(
        &mut self,
        ord: <C as component::Multi>::Ord,
        entity: Entity<A>,
        f: F,
    ) -> Option<R>
    where
        A: archetype::Contains<C>,
    {
        let storage = &self.get_table_mut::<A>().multi_column_mut::<C>(ord).storage;
        let mut storage = storage.write().expect("Another thread panicked");
        Some(f(storage.get_mut(entity.id)?))
    }
}

/// Stores all components for the same archetype.
pub struct Table<A: Archetype> {
    entities: entity::Counter,
    multi_ctors: BTreeMap<TypeId, Box<dyn Fn() -> Arc<dyn AnyColumn>>>,
    columns: BTreeMap<ColumnId, Arc<dyn AnyColumn>>,
    _ph: PhantomData<&'static A>,
}

impl<A: Archetype> Default for Table<A> {
    fn default() -> Self {
        Table {
            entities: entity::Counter::default(),
            multi_ctors: BTreeMap::new(),
            columns: BTreeMap::new(),
            _ph: PhantomData,
        }
    }
}

impl<A: Archetype> Table<A> {
    /// Retrieves a specific column for a specific single-component.
    pub fn single_column<C: component::Single>(&self) -> &Column<A, C>
    where
        A: archetype::Contains<C>,
    {
        if let Some(column) = self.columns.get(&ColumnId::single::<C>()) {
            column
                .as_any()
                .downcast_ref::<Column<A, C>>()
                .expect("TypeId mismatch")
        } else {
            panic!(
                "Component type {c} was not registered for archetype {a}",
                a = any::type_name::<A>(),
                c = any::type_name::<C>()
            )
        }
    }

    /// Retrieves a specific column for a specific single-component.
    pub fn single_column_mut<C: component::Single>(&mut self) -> &mut Column<A, C>
    where
        A: archetype::Contains<C>,
    {
        if let Some(column) = self.columns.get_mut(&ColumnId::single::<C>()) {
            let column = Arc::get_mut(column)
                .expect("Arc<dyn AnyColumn> should be dropped before cycle ends");

            column
                .as_any_mut()
                .downcast_mut::<Column<A, C>>()
                .expect("TypeId mismatch")
        } else {
            panic!(
                "Component type {c} was not registered for archetype {a}",
                a = any::type_name::<A>(),
                c = any::type_name::<C>()
            )
        }
    }

    /// Retrieves the column for a specific multi-component.
    pub fn multi_column<C: component::Multi>(
        &self,
        ord: <C as component::Multi>::Ord,
    ) -> Option<&Column<A, C>>
    where
        A: archetype::Contains<C>,
    {
        let id = ColumnId::multi::<C>(ord.into());
        let column = self.columns.get(&id)?;
        Some(
            column
                .as_any()
                .downcast_ref::<Column<A, C>>()
                .expect("TypeId mismatch"),
        )
    }

    /// Retrieves the column for a specific multi-component.
    pub fn multi_column_mut<C: component::Multi>(
        &mut self,
        ord: <C as component::Multi>::Ord,
    ) -> &mut Column<A, C>
    where
        A: archetype::Contains<C>,
    {
        let id = ColumnId::multi::<C>(ord.into());
        let column = self.columns.entry(id).or_insert_with(|| {
            let factory = match self.multi_ctors.get(&id.ty) {
                Some(factory) => factory,
                None => panic!(
                    "Component type {c} was not registered for archetype {a}",
                    a = any::type_name::<A>(),
                    c = any::type_name::<C>()
                ),
            };
            factory()
        });

        let column =
            Arc::get_mut(column).expect("Arc<dyn AnyColumn> should be dropped before cycle ends");

        column
            .as_any_mut()
            .downcast_mut::<Column<A, C>>()
            .expect("TypeId mismatch")
    }

    /// Inserts a new entity into the table.
    ///
    /// Requires exclusive access to the table.
    pub fn insert(
        &mut self,
        near: Option<entity::Weak<A>>,
        mut initials: component::Initials<A>,
    ) -> Entity<A> {
        let entity = Entity::new(self.entities.allocate(near.map(|weak| weak.id)));

        for (&key, column) in &mut self.columns {
            let column = Arc::get_mut(column)
                .expect("Arc<dyn AnyColumn> should be dropped before cycle ends");

            match key.ord {
                ColumnIdOrd::Single => {
                    let initial = initials.single.remove(&key.ty);
                    column.init(entity.id, initial.map(|entry| entry.value));
                }
                // multi columns are automatically optional;
                // Assign them by iterating over `initials.multi` later on.
                ColumnIdOrd::Multi(..) => {}
            }
        }

        if !initials.single.is_empty() {
            let types = initials
                .single
                .values()
                .map(|entry| entry.type_name)
                .join(", ");
            panic!(
                "Component types {} were not registered for archetype {}",
                types,
                any::type_name::<A>()
            );
        }

        for (component_ty, entry) in initials.multi.into_iter() {
            for (ord, value) in entry.values {
                let column_id = ColumnId {
                    ty: component_ty,
                    ord: ColumnIdOrd::Multi(ord),
                    ty_name: entry.type_name,
                };

                let column = self.columns.entry(column_id).or_insert_with(|| {
                    let factory = match self.multi_ctors.get(&component_ty) {
                        Some(factory) => factory,
                        None => panic!(
                            "Component type {c} was not registered for archetype {a}",
                            a = any::type_name::<A>(),
                            c = entry.type_name,
                        ),
                    };
                    factory()
                });

                let column = Arc::get_mut(column)
                    .expect("Arc<dyn AnyColumn> should be dropped before cycle ends");
                column.init(entity.id, Some(value));
            }
        }

        entity
    }

    /// Removes a new entity from the table.
    ///
    /// Requires exclusive access to the table.
    pub fn remove(&mut self, entity: Entity<A>) {
        if entity.ref_count() > 1 {
            panic!(
                "Detected {} dangling references to entity {:?}",
                entity.ref_count() - 1,
                entity
            );
        }

        self.entities.delete(entity.id);
    }
}

/// Identifies a component, and the instance ord if it is a multi-component.
#[derive(Debug, Clone, Copy)]
struct ColumnId {
    ty: TypeId,
    ord: ColumnIdOrd,
    ty_name: &'static str,
}

impl PartialEq for ColumnId {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
            && match (self.ord, other.ord) {
                (ColumnIdOrd::Single, ColumnIdOrd::Single) => true,
                (ColumnIdOrd::Multi(a), ColumnIdOrd::Multi(b)) => a == b,
                (ColumnIdOrd::Single, ColumnIdOrd::Multi(..))
                | (ColumnIdOrd::Multi(..), ColumnIdOrd::Single) => {
                    unreachable!("Column type and ord variant mismatch")
                }
            }
    }
}

impl Eq for ColumnId {}

impl PartialOrd for ColumnId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ColumnId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ty
            .cmp(&other.ty)
            .then_with(|| match (self.ord, other.ord) {
                (ColumnIdOrd::Single, ColumnIdOrd::Single) => std::cmp::Ordering::Equal,
                (ColumnIdOrd::Multi(a), ColumnIdOrd::Multi(b)) => a.cmp(&b),
                (ColumnIdOrd::Single, ColumnIdOrd::Multi(..))
                | (ColumnIdOrd::Multi(..), ColumnIdOrd::Single) => {
                    unreachable!("Column type and ord variant mismatch")
                }
            })
    }
}

#[derive(Debug, Clone, Copy)]
enum ColumnIdOrd {
    Single,
    Multi(usize),
}

impl ColumnId {
    fn single<C: component::Single>() -> Self {
        Self {
            ty: TypeId::of::<C>(),
            ord: ColumnIdOrd::Single,
            ty_name: any::type_name::<C>(),
        }
    }

    fn multi<C: component::Multi>(ord: usize) -> Self {
        Self {
            ty: TypeId::of::<C>(),
            ord: ColumnIdOrd::Multi(ord),
            ty_name: any::type_name::<C>(),
        }
    }
}
