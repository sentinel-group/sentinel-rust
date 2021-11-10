#![allow(unused_macros)]

// crate `cfg_if` cannot be used in stable toolchains (due to `expr`),
// so currently we use `stmt` instead
#[macro_export]
macro_rules! cfg_if_async {
    ($item1:stmt, $item2:stmt) => {
        #[cfg(feature = "async")]
        $item1;
        #[cfg(not(feature = "async"))]
        $item2;
    };
}

macro_rules! cfg_async {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "async")]
            $item
        )*
    }
}

macro_rules! cfg_not_async {
    ($($item:item)*) => {
        $(
            #[cfg(not(feature = "async"))]
            $item
        )*
    }
}

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
