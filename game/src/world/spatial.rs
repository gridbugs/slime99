spatial_table::declare_layers_module! {
    layers {
        floor: Floor,
        feature: Feature,
        character: Character,
    }
}
pub use layers::{Layer, Layers};
pub type SpatialTable = spatial_table::SpatialTable<Layers>;
pub type Location = spatial_table::Location<Layer>;
pub use spatial_table::UpdateError;
