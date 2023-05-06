use std::rc::Rc;

use crate::math;
use crate::vertex::Vertex;

pub struct GameObject {
    transform: math::TransformMat,
    mesh: Rc<Mesh>,
}

pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}