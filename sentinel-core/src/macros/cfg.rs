#![allow(unused_macros)]

macro_rules! cfg_exporter {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "exporter")]
            #[cfg_attr(docsrs, doc(cfg(feature = "exporter")))]
            $item
        )*
    }
}

macro_rules! cfg_datasource {
    ($($item:item)*) => {
        $(
            #[cfg(any(feature = "ds_etcdv3", feature = "ds_consul", feature = "ds_k8s"))]
            #[cfg_attr(docsrs, doc(cfg(any(feature = "ds_etcdv3", feature = "ds_consul", feature = "ds_k8s"))))]
            $item
        )*
    }
}

macro_rules! cfg_k8s {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "ds_k8s")]
            $item
        )*
    }
}
