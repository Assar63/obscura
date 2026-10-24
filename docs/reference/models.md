# OBSBOT model and feature matrix

This matrix separates what OBSCura has tested from what it merely inherits
or what libdev 2.1.0 declares. A model appearing in the SDK is not evidence
that its features work with OBSCura.

## Evidence key

- **Tested** — feature behavior was measured on a real camera; see the
  [project feature list](../../README.md#features) and
  [protocol reference](protocol.md).
- **Assumed** — a profile inherits bindings from another model, but the
  inherited behavior has not been verified on this model.
- **SDK lead** — the SDK header declares the feature or model family; no
  camera-specific trace and hardware test has been completed here.
- **UVC only** — OBSCura currently exposes standard webcam controls, without
  a model-specific vendor profile.

## Models with OBSCura profiles

| Model | USB ID | Gimbal controls | AI tracking and gestures | Model-specific features | OBSCura evidence |
|---|---|---|---|---|---|
| Tiny SE | `3564:feff` | Gimbal, joystick, reset, presets — **Tested** | Tracking modes and gesture controls — **Tested** | Image, audio and device controls — see [feature list](../../README.md#features) | **Tested** using OBSBOT Center captures; firmware 6.4.4.1 |
| Tiny 3 | `3564:ff02` | Gimbal, joystick, reset, presets, FOV and preset speed — **Tested** | Human, Group, Hand, Whiteboard, Desk and Voice Tracking; gesture controls — **Tested** | Vox SE wireless microphones, voice control, DOA tracking and camera events — **Tested** | **Tested** on firmware 6.6.8.3; feature-by-feature evidence is in the [roadmap](../project/roadmap.md) and [protocol reference](protocol.md) |
| Tiny 3 Lite | `3564:ff04` | Inherits Tiny 3 bindings — **Assumed** | Inherits Tiny 3 bindings — **Assumed** | Inherits Tiny 3 bindings — **Assumed** | Profile exists; no direct hardware verification |
| Tiny 2 | `3564:fef8` | Inherits Tiny SE bindings — **Assumed** | Inherits Tiny SE bindings — **Assumed** | Inherits Tiny SE bindings — **Assumed** | Profile exists; several protocol IDs cross-checked against independent Tiny 2 research |
| Tiny 2 Lite | `3564:fef9` | Inherits Tiny SE bindings — **Assumed** | Inherits Tiny SE bindings — **Assumed** | Inherits Tiny SE bindings — **Assumed** | Profile exists; unverified |
| Tiny 4K | `3564:fef4` | Inherits Tiny SE bindings — **Assumed** | Inherits Tiny SE bindings — **Assumed** | Inherits Tiny SE bindings — **Assumed** | Profile exists; unverified |
| Meet 2 | `3564:fefb` | Gimbal controls dropped from inherited profile | Inherits Tiny SE bindings — **Assumed** | Inherits Tiny SE bindings — **Assumed** | Profile exists; unverified |
| Meet SE | `3564:fefe` | Gimbal controls dropped from inherited profile | Inherits Meet 2 / Tiny SE bindings — **Assumed** | Inherits Meet 2 / Tiny SE bindings — **Assumed** | Profile exists; unverified |

Inherited bindings indicate what OBSCura will currently offer, not guaranteed
hardware support. The [README](../../README.md#supported-cameras) summarizes
the evidence and the supported-camera status.

## Other camera models named by libdev 2.1.0

The SDK header recognizes these additional OBSBOT camera models. They do not
have a model-specific OBSCura profile in this repository; the app currently
uses generic UVC controls where applicable. SDK feature declarations below
are research leads only.

| Model | SDK-declared feature area | OBSCura status |
|---|---|---|
| Tiny (original) | Tiny-family APIs are present in the SDK | **UVC only**; no model-specific profile |
| Meet | Meet-family media mode, auto-framing, and virtual-background APIs | **SDK lead**; no model-specific profile or hardware verification |
| Meet 4K | Meet-family media mode, auto-framing, and virtual-background APIs | **SDK lead**; no model-specific profile or hardware verification |
| Me | Product type is named in the SDK | **UVC only**; no model-specific profile |
| Tail Air | Tail-family tracking and gimbal APIs | No model-specific profile; separate product/API family |
| Tail 2 | Tail-family AI mode and tracking APIs | No model-specific profile; separate product/API family |
| Tail 2S | Tail-family AI mode and tracking APIs | No model-specific profile; separate product/API family |

The SDK's Meet / Meet 4K declarations are described in more detail in the
[roadmap](../project/roadmap.md#5-sdk-leads-for-meet-and-meet-4k). They must
not be assumed to apply to Meet 2 or Meet SE: libdev describes those models
using a different status layout.

The SDK also names HDMI and NDI boxes, which are not camera models and are
outside this camera feature matrix. Product names, API categories, and
status structures are not substitutes for a model-specific trace and
hardware test.
