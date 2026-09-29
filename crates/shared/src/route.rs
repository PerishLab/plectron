#[derive(Debug, PartialEq, Eq)]
pub enum Route<T> {
    Reuse,
    Update(T),
    Build,
}

pub fn select<T>(reusable: bool, updates: bool, declare: impl FnOnce() -> Option<T>) -> Route<T> {
    if reusable {
        Route::Reuse
    } else if updates {
        declare().map_or(Route::Build, Route::Update)
    } else {
        Route::Build
    }
}
