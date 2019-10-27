use std::fs::File;
use std::io::BufReader;
use std::collections::HashMap;
use std::ops::Sub;
use std::io::prelude::*;

#[macro_use] extern crate lazy_static;
extern crate regex;
use regex::Regex;

lazy_static! {
    static ref OSM_NODE_RE: Regex = (
        Regex::new(r#"id="(\d+)" lat="([0-9.]+)" lon="([0-9.]+)""#).unwrap()
    );
    static ref OSM_HIGHWAY_RE: Regex = Regex::new(r#"k="highway" v="([a-z_]+)""#).unwrap();
    static ref OSM_ND_RE: Regex = Regex::new(r#"<nd ref="(\d+)""#).unwrap();
}

const KMPH: f32 = 1000_f32 / 3600_f32;  // km/h to m/s factor


#[derive(Debug, Copy, Clone)]
struct Arc {
    index: usize,
    cost: usize,  // in seconds
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct Point {
    lat: f32,
    lon: f32,
}

#[derive(Debug)]
struct RoadNetwork {
    osm_id_map: HashMap<isize, usize>,
    nodes: HashMap<isize, Point>,
    adjacent_arcs: Vec<Vec<Arc>>,
}

impl Sub for Point {
    type Output = f32;

    fn sub(self, other: Point) -> f32 {
        (
            ((self.lat - other.lat) * 111_229_f32).powi(2) +
            ((self.lon - other.lon) * 71_695_f32).powi(2)
        ).sqrt()
    }
}

impl RoadNetwork {
    pub fn new() -> RoadNetwork {
        RoadNetwork {
            osm_id_map: HashMap::new(),
            nodes: HashMap::new(),
            adjacent_arcs: Vec::new(),
        }
    }

    pub fn add_node(&mut self, osm_id: isize, location: Point) {
        self.nodes.insert(osm_id, location);
    }

    pub fn get_index(&self, osm_id: isize) -> Option<usize> {
        match self.osm_id_map.get(&osm_id) {
            Some(osm_id) => Some(*osm_id),
            None => None,
        }
    }

    pub fn get_or_create_index(&mut self, osm_id: isize) -> usize {
        match self.get_index(osm_id) {
            Some(index) => index,
            None => {
                let index = self.adjacent_arcs.len();
                self.adjacent_arcs.push(Vec::new());
                self.osm_id_map.insert(osm_id, index);
                index
            }
        }
    }

    pub fn distance(&self, osm_id_a: isize, osm_id_b: isize) -> f32 {
        let location_a = *self.nodes.get(&osm_id_a).unwrap();
        let location_b = *self.nodes.get(&osm_id_b).unwrap();
        location_a - location_b
    }

    fn _push_arc_at_index(&mut self, index: usize, arc: Arc) {
        let node = self.adjacent_arcs.get_mut(index).unwrap();
        node.push(arc);
    }

    pub fn add_arc(&mut self, osm_id_a: isize, osm_id_b: isize, speed_factor: f32) {
        let cost = (self.distance(osm_id_a, osm_id_b) / speed_factor) as usize;
        let index_a = self.get_or_create_index(osm_id_a);
        let index_b = self.get_or_create_index(osm_id_b);
        self._push_arc_at_index(index_a, Arc {index: index_b, cost});
        self._push_arc_at_index(index_b, Arc {index: index_a, cost});
    }

    pub fn read_from_osm_file(&mut self, filename: &str) -> std::io::Result<()>{
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let mut hops: Vec<isize> = Vec::new();
        let mut is_way = false;
        let mut is_highway = false;
        let mut speed_factor = 0_f32;

        for line in reader.lines() {
            if let Ok(line) = line {
                let trimmed_line = line.trim_start();
                if let Some(cap) = OSM_NODE_RE.captures(trimmed_line) {
                    self.add_node(
                        cap[1].parse::<isize>().unwrap(),
                        Point{
                            lat: cap[2].parse::<f32>().unwrap(),
                            lon: cap[3].parse::<f32>().unwrap()
                        }
                    );
                } else if trimmed_line.starts_with(r"<way ") {
                    hops = Vec::new();
                    is_way = true;
                    is_highway = false;
                } else if is_way {
                    if let Some(cap) = OSM_ND_RE.captures(trimmed_line) {
                        hops.push(cap[1].parse::<isize>().unwrap());
                    } else if let Some(cap) = OSM_HIGHWAY_RE.captures(trimmed_line) {
                        is_highway = true;
                        speed_factor = KMPH * match &cap[1] {
                            "motorway" => 110_f32,
                            "trunk" => 110_f32,
                            "primary" => 70_f32,
                            "secondary" => 60_f32,
                            "tertiary" => 50_f32,
                            "motorway_link" => 50_f32,
                            "trunk_link" => 50_f32,
                            "primary_link" => 50_f32,
                            "secondary_link" => 50_f32,
                            "road" => 40_f32,
                            "unclassified" => 40_f32,
                            "residential" => 30_f32,
                            "unsurfaced" => 30_f32,
                            "living_street" => 10_f32,
                            "service" => 5_f32,
                            &_ => {
                                is_highway = false;
                                0_f32
                            }
                        };
                    } else if trimmed_line.starts_with(r"</way") {
                        if is_highway && speed_factor > 0_f32{
                            let mut previous = 0;
                            for hop in hops.clone() {
                                if previous > 0 {
                                    self.add_arc(hop, previous, speed_factor);
                                }
                                previous = hop;
                            }
                        }
                        is_way = false;
                    }
                }
            }
        }
        Ok(())
    }
}

fn main() -> std::io::Result<()> {
    let mut road_network = RoadNetwork::new();
    road_network.read_from_osm_file("saarland.osm")?;
    println!("{:?}", road_network.adjacent_arcs);
    Ok(())
}
