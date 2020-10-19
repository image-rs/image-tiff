use std::{collections::HashMap, convert::TryFrom};
// use tags::Tag;

macro_rules! enum_try_from {
    {#[$enum_attr:meta] $vis:vis enum $tyname:ident { $($name:ident = $value:expr),* $(,)* }} => {
        #[$enum_attr]
        $vis enum $tyname {
            $($name = $value,)*
        }

        impl ::std::convert::TryFrom<usize> for $tyname {
            type Error = ();
            /// Get enum key for value
            fn try_from(num: usize) -> Result<Self, ()> {
                $(
                    if num == $value {
                        Ok($tyname::$name)
                    } else
                )* {
                    Err(())
                }
            }
        }
    };
}

// More info about geotiff keys can be found
// [here](http://geotiff.maptools.org/spec/geotiff2.7.html)
enum_try_from! {
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub enum GeoKey {
    // GeoTIFF Configuration GeoKeys
    GTModelTypeGeoKey = 1024,
    GTRasterTypeGeoKey = 1025,
    GTCitationGeoKey = 1026,
    // Geographic CS Parameter GeoKeys
    GeographicTypeGeoKey = 2048,
    GeogCitationGeoKey = 2049,
    GeogGeodeticDatumGeoKey = 2050,
    GeogPrimeMeridianGeoKey = 2051,
    GeogLinearUnitsGeoKey = 2052,
    GeogLinearUnitSizeGeoKey = 2053,
    GeogAngularUnitsGeoKey = 2054,
    GeogAngularUnitSizeGeoKey = 2055,
    GeogEllipsoidGeoKey = 2056,
    GeogSemiMajorAxisGeoKey = 2057,
    GeogSemiMinorAxisGeoKey = 2058,
    GeogInvFlatteningGeoKey = 2059,
    GeogAzimuthUnitsGeoKey = 2060,
    GeogPrimeMeridianLongGeoKey = 2061,
    // Projected CS Parameter GeoKeys
    ProjectedCSTypeGeoKey = 3072,
    PCSCitationGeoKey = 3073,
    // Projection Definition GeoKeys
    ProjectionGeoKey = 3074,
    ProjCoordTransGeoKey = 3075,
    ProjLinearUnitsGeoKey = 3076,
    ProjLinearUnitSizeGeoKey = 3077,
    ProjStdParallel1GeoKey = 3078,
    ProjStdParallel2GeoKey = 3079,
    ProjNatOriginLongGeoKey = 3080,
    ProjNatOriginLatGeoKey = 3081,
    ProjFalseEastingGeoKey = 3082,
    ProjFalseNorthingGeoKey = 3083,
    ProjFalseOriginLongGeoKey = 3084,
    ProjFalseOriginLatGeoKey = 3085,
    ProjFalseOriginEastingGeoKey = 3086,
    ProjFalseOriginNorthingGeoKey = 3087,
    ProjCenterLongGeoKey = 3088,
    ProjCenterLatGeoKey = 3089,
    ProjCenterEastingGeoKey = 3090,
    ProjCenterNorthingGeoKey = 3091,
    ProjScaleAtNatOriginGeoKey = 3092,
    ProjScaleAtCenterGeoKey = 3093,
    ProjAzimuthAngleGeoKey = 3094,
    ProjStraightVertPoleLongGeoKey = 3095,
    // Vertical CS Parameter Keys
    VerticalCSTypeGeoKey = 4096,
    VerticalCitationGeoKey = 4097,
    VerticalDatumGeoKey = 4098,
    VerticalUnitsGeoKey = 4099,
}
}
#[derive(Clone, Debug)]
pub enum GeoKeyType {
    Short(u16),
    Double(Vec<f64>),
    Ascii(String),
    ShortVec(Vec<u16>),
}

pub fn get_geo_key(geodir: HashMap<GeoKey, GeoKeyType>, key: &GeoKey) -> Option<GeoKeyType> {
    let res = geodir.get(key);

    return match res {
        Some(val) => Some(val.clone()),
        None => None,
    };
}

pub fn parse_geo_keys(
    geokey_dir: Vec<u16>,
    ascii_params: Option<String>,
    double_params: Option<Vec<f64>>,
) -> HashMap<GeoKey, GeoKeyType> {
    let mut geodir: HashMap<GeoKey, GeoKeyType> = HashMap::new();
    if geokey_dir.len() < 4 {
        // TODO: or return an error in a result
        return geodir;
    }

    let num_keys = geokey_dir[3] as usize;

    for i in 0..num_keys {
        let idx = 4 + i * 4;
        // Each keyEntry is made up of SHORTS: KeyID, TIFFTagLocation, Count, Value_Offset
        let key_id = match GeoKey::try_from(geokey_dir[idx] as usize) {
            Ok(value) => value,
            Err(_) => {
                continue;
            }
        };
        let tiff_tag_location = geokey_dir[idx + 1];
        let count = geokey_dir[idx + 2];
        let value_offset = geokey_dir[idx + 3];

        if tiff_tag_location == 0 {
            geodir.insert(key_id, GeoKeyType::Short(value_offset as u16));
        } else if tiff_tag_location == 34736 {
            // should be accessed rather by Tag::GeoDoubleParamsTag
            match &double_params {
                Some(double_params) => {
                    let start = value_offset as usize;
                    let stop = (value_offset + count) as usize;

                    if start > double_params.len() || stop > double_params.len() {
                        continue;
                    }
                    let mut value = vec![0.; count as usize];
                    value.copy_from_slice(&double_params[start..stop]);
                    geodir.insert(key_id, GeoKeyType::Double(value));
                }
                None => continue,
            }
        } else if tiff_tag_location == 34737 {
            // should be accessed rather by Tag::GeoAsciiParamsTag
            match &ascii_params {
                Some(ascii_params) => {
                    let start = value_offset as usize;
                    let stop = (value_offset + count) as usize;

                    if start > ascii_params.len() || stop > ascii_params.len() {
                        continue;
                    }
                    let value = String::from(&ascii_params[start..stop]);
                    geodir.insert(key_id, GeoKeyType::Ascii(value));
                }
                None => continue,
            }
        } else if tiff_tag_location == 34735 {
            // should be accessed rather by Tag::GeoKeyDirectoryTag
            // If the tag is the same as the GeoKeyDirectoryTag, the value_offset represents
            // SHORT values at the end of the `geokey_dir` itself.
            let start = value_offset as usize;
            let stop = (value_offset + count) as usize;

            if start > geokey_dir.len() || stop > geokey_dir.len() {
                continue;
            }

            let mut value = vec![0; count as usize];
            value.copy_from_slice(&geokey_dir[start..stop]);
            geodir.insert(key_id, GeoKeyType::ShortVec(value));
        } else {
            // undefined
            continue;
        }
    }
    geodir
}
