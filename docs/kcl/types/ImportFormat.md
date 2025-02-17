---
title: "ImportFormat"
excerpt: "Import format specifier"
layout: manual
---

Import format specifier





**This schema accepts exactly one of the following:**

Autodesk Filmbox (FBX) format

**Type:** `object`





## Properties

| Property | Type | Description | Required |
|----------|------|-------------|----------|
| `format` |enum: `fbx`|  | No |


----
Binary glTF 2.0. We refer to this as glTF since that is how our customers refer to it, but this can also import binary glTF (glb).

**Type:** `object`





## Properties

| Property | Type | Description | Required |
|----------|------|-------------|----------|
| `format` |enum: `gltf`|  | No |


----
Wavefront OBJ format.

**Type:** `object`





## Properties

| Property | Type | Description | Required |
|----------|------|-------------|----------|
| `format` |enum: `obj`|  | No |
| `coords` |[`System`](/docs/kcl/types/System)| Co-ordinate system of input data. Defaults to the [KittyCAD co-ordinate system. | No |
| `units` |[`UnitLength`](/docs/kcl/types/UnitLength)| The units of the input data. This is very important for correct scaling and when calculating physics properties like mass, etc. Defaults to millimeters. | No |


----
The PLY Polygon File Format.

**Type:** `object`





## Properties

| Property | Type | Description | Required |
|----------|------|-------------|----------|
| `format` |enum: `ply`|  | No |
| `coords` |[`System`](/docs/kcl/types/System)| Co-ordinate system of input data. Defaults to the [KittyCAD co-ordinate system. | No |
| `units` |[`UnitLength`](/docs/kcl/types/UnitLength)| The units of the input data. This is very important for correct scaling and when calculating physics properties like mass, etc. Defaults to millimeters. | No |


----
SolidWorks part (SLDPRT) format.

**Type:** `object`





## Properties

| Property | Type | Description | Required |
|----------|------|-------------|----------|
| `format` |enum: `sldprt`|  | No |


----
ISO 10303-21 (STEP) format.

**Type:** `object`





## Properties

| Property | Type | Description | Required |
|----------|------|-------------|----------|
| `format` |enum: `step`|  | No |


----
ST**ereo**L**ithography format.

**Type:** `object`





## Properties

| Property | Type | Description | Required |
|----------|------|-------------|----------|
| `format` |enum: `stl`|  | No |
| `coords` |[`System`](/docs/kcl/types/System)| Co-ordinate system of input data. Defaults to the [KittyCAD co-ordinate system. | No |
| `units` |[`UnitLength`](/docs/kcl/types/UnitLength)| The units of the input data. This is very important for correct scaling and when calculating physics properties like mass, etc. Defaults to millimeters. | No |


----




