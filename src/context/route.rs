/// Which screen is active. Mirrors upstream `context/route.tsx`, limited to the
/// M1 screens.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Route {
    #[default]
    Home,
    Session(String),
}
