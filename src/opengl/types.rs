use glcore::GLCoreError;

pub type GlResult<T> = Result<T, GLCoreError>;

pub trait VecPromotion<P> {
    fn promote(self, n: f32) -> P;
    fn promote_zero(self) -> P;
}

pub trait VecDemotion<D> {
    fn demote(self) -> D;
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Vec2 {
    x: f32,
    y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }
}

impl VecPromotion<Vec3> for Vec2 {
    fn promote(self, z: f32) -> Vec3 {
        Vec3 {
            x: self.x,
            y: self.y,
            z,
        }
    }

    fn promote_zero(self) -> Vec3 {
        Vec3 {
            x: self.x,
            y: self.y,
            z: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3 { x, y, z }
    }
}

impl VecPromotion<Vec4> for Vec3 {
    fn promote(self, w: f32) -> Vec4 {
        Vec4 {
            x: self.x,
            y: self.y,
            z: self.z,
            w,
        }
    }

    fn promote_zero(self) -> Vec4 {
        Vec4 {
            x: self.x,
            y: self.y,
            z: self.z,
            w: 0.0,
        }
    }
}

impl VecDemotion<Vec2> for Vec3 {
    fn demote(self) -> Vec2 {
        Vec2 {
            x: self.x,
            y: self.y,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Vec4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

impl Vec4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Vec4 {
        Vec4 { x, y, z, w }
    }
}

impl VecDemotion<Vec3> for Vec4 {
    fn demote(self) -> Vec3 {
        Vec3 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }
}

pub trait AsFloatArray {
    const FLOATS_PER_ELEMENT: usize;
    type Backend;

    fn as_contiguous_block(&self) -> Option<&[f32]>;
}

macro_rules! make_continguous {
    ($borrowed:ident, $collection:ident, $singular:ty, $per_elem:expr) => {
        #[repr(transparent)]
        pub struct $collection(Vec<$singular>);

        impl $collection {
            pub fn new(verts: Vec<$singular>) -> $collection {
                $collection(verts)
            }
        }

        impl AsFloatArray for $collection {
            const FLOATS_PER_ELEMENT: usize = $per_elem;
            type Backend = $singular;

            fn as_contiguous_block(&self) -> Option<&[f32]> {
                if self.0.len() == 0 {
                    return None;
                }
                Some(unsafe {
                    std::slice::from_raw_parts(
                        self.0.as_ptr() as *const f32,
                        self.0.len() * Self::FLOATS_PER_ELEMENT,
                    )
                })
            }
        }

        impl<'a> $collection {
            pub fn weaken(&'a self) -> $borrowed<'a> {
                $borrowed::new(&self.0)
            }
        }
    };

    ($collection:ident, $singular:ty, $per_elem:expr) => {
        #[repr(transparent)]
        pub struct $collection<'a>(&'a [$singular]);

        impl<'a> $collection<'a> {
            pub fn new(verts: &'a [$singular]) -> $collection<'a> {
                $collection(verts)
            }
        }

        impl<'a> AsFloatArray for $collection<'a> {
            const FLOATS_PER_ELEMENT: usize = $per_elem;
            type Backend = $singular;

            fn as_contiguous_block(&self) -> Option<&[f32]> {
                if self.0.len() == 0 {
                    return None;
                }
                Some(unsafe {
                    std::slice::from_raw_parts(
                        self.0.as_ptr() as *const f32,
                        self.0.len() * Self::FLOATS_PER_ELEMENT,
                    )
                })
            }
        }
    };
}

make_continguous!(Vec2Array, Vec2, 2);
make_continguous!(Vec3Array, Vec3, 3);
make_continguous!(Vec4Array, Vec4, 4);
make_continguous!(Vec2Array, OwnedVec2Array, Vec2, 2);
make_continguous!(Vec3Array, OwnedVec3Array, Vec3, 3);
make_continguous!(Vec4Array, OwnedVec4Array, Vec4, 4);
