// use std::{
//     cell::RefCell,
//     collections::HashMap,
//     rc::{Rc, Weak},
// };

// use gulf_core::rc::try_new_cyclic;
// use pipewire::{
//     context::ContextRc,
//     core::CoreRc,
//     keys::{MEDIA_CATEGORY, MEDIA_ROLE, MEDIA_TYPE},
//     main_loop::MainLoopRc,
//     properties::{PropertiesBox, properties as props},
//     registry::{self, GlobalObject, RegistryRc},
//     types::ObjectType,
// };

// pub struct CaptureState {
//     pub(crate) main_loop: MainLoopRc,
//     pub(crate) context: ContextRc,
//     pub(crate) core: CoreRc,
//     pub(crate) registry: RegistryRc,
//     pub(crate) registry_listener: registry::Listener,
//     pub(crate) globals: RefCell<HashMap<u32, GlobalObject<PropertiesBox>>>,
// }

// impl CaptureState {
//     pub fn new() -> Result<Rc<Self>, pipewire::Error> {
//         try_new_cyclic(|state: &Weak<CaptureState>| {
//             let main_loop = MainLoopRc::new(None)?;
//             let context = ContextRc::new(&main_loop, None)?;
//             let core = context.connect_rc(None)?;
//             let registry = core.get_registry_rc()?;
//             let registry_listener = registry
//                 .add_listener_local()
//                 .global({
//                     let state = state.clone();

//                     move |added| {
//                         let Some(state) = state.upgrade() else {
//                             println!("state no longer exists");
//                             return;
//                         };

//                         if added.type_ != ObjectType::Node {
//                             return;
//                         }

//                         println!("global added: {added:#?}");
//                         state
//                             .globals
//                             .borrow_mut()
//                             .insert(added.id, added.to_owned());
//                     }
//                 })
//                 .global_remove({
//                     let state = state.clone();
//                     move |removed| {
//                         println!("global removed: {removed:?}");
//                         let Some(state) = state.upgrade() else {
//                             println!("state no longer exists");
//                             return;
//                         };

//                         state.globals.borrow_mut().remove(&removed);
//                     }
//                 })
//                 .register();

//             Ok(CaptureState {
//                 main_loop,
//                 context,
//                 core,
//                 registry,
//                 registry_listener,
//                 globals: RefCell::new(HashMap::new()),
//             })
//         })
//     }
// }

// #[cfg(test)]
// #[test]
// fn lol() {
//     let state = CaptureState::new().expect("failed to lol");

//     state.main_loop.run();
// }
