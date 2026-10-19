//! Same-HWND backdrop visuals, behind the WebView. No screen readback or capture exclusion.
#![allow(non_snake_case)] // Windows COM implementation method names.

use super::liquid_glass::BackdropSurface;
use windows::{
    core::{implement, Interface, Result, HSTRING, PCWSTR, GUID, BOOL},
    Foundation::{IPropertyValue, PropertyValue},
    Graphics::{IGeometrySource2D, IGeometrySource2D_Impl, Effects::*},
    System::{DispatcherQueue, DispatcherQueueController},
    UI::Composition::{Compositor, ContainerVisual, CompositionPath, CompositionEffectSourceParameter, Desktop::DesktopWindowTarget},
    Win32::{
        Foundation::{HWND, E_INVALIDARG, E_NOTIMPL},
        Graphics::{Dwm::*, Direct2D::{*, Common::*}},
        System::WinRT::{*, Composition::ICompositorDesktopInterop, Graphics::Direct2D::*},
    },
};

#[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
struct BlurEffect { amount: f32, source: IGraphicsEffectSource }
impl IGraphicsEffectSource_Impl for BlurEffect_Impl {}
impl IGraphicsEffect_Impl for BlurEffect_Impl {
    fn Name(&self) -> Result<HSTRING> { Ok("EconomyWarRoomBlur".into()) }
    fn SetName(&self, _: &HSTRING) -> Result<()> { Err(E_NOTIMPL.into()) }
}
impl IGraphicsEffectD2D1Interop_Impl for BlurEffect_Impl {
    fn GetEffectId(&self) -> Result<GUID> { Ok(CLSID_D2D1GaussianBlur) }
    fn GetNamedPropertyMapping(&self, _: &PCWSTR, _: *mut u32, _: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING) -> Result<()> {
        Err(E_INVALIDARG.into())
    }
    fn GetPropertyCount(&self) -> Result<u32> { Ok(3) }
    fn GetProperty(&self, index: u32) -> Result<IPropertyValue> {
        match index {
            0 => PropertyValue::CreateSingle(self.amount)?.cast(),
            1 => PropertyValue::CreateUInt32(D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED.0 as u32)?.cast(),
            2 => PropertyValue::CreateUInt32(D2D1_BORDER_MODE_HARD.0 as u32)?.cast(),
            _ => Err(E_INVALIDARG.into()),
        }
    }
    fn GetSource(&self, index: u32) -> Result<IGraphicsEffectSource> {
        if index == 0 { Ok(self.source.clone()) } else { Err(E_INVALIDARG.into()) }
    }
    fn GetSourceCount(&self) -> Result<u32> { Ok(1) }
}

#[implement(IGeometrySource2D, IGeometrySource2DInterop)]
struct GeometrySource(ID2D1Geometry);
impl IGeometrySource2D_Impl for GeometrySource_Impl {}
impl IGeometrySource2DInterop_Impl for GeometrySource_Impl {
    fn GetGeometry(&self) -> Result<ID2D1Geometry> { Ok(self.0.clone()) }
    fn TryGetGeometryUsingFactory(&self, _: windows_core::Ref<'_, ID2D1Factory>) -> Result<ID2D1Geometry> {
        Err(E_NOTIMPL.into())
    }
}

pub struct NativeMaterial {
    compositor: Compositor,
    target: DesktopWindowTarget,
    root: ContainerVisual,
    _queue: Option<DispatcherQueueController>,
    surfaces: Vec<BackdropSurface>,
}
impl NativeMaterial {
    // Construct and call only from Tauri's UI thread, where the dispatcher lives.
    pub fn new(hwnd: HWND) -> Result<Self> {
        let queue = if DispatcherQueue::GetForCurrentThread().is_err() {
            Some(unsafe { CreateDispatcherQueueController(DispatcherQueueOptions {
                dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT, apartmentType: DQTAT_COM_ASTA,
            })? })
        } else { None };
        let enabled = BOOL(1);
        unsafe { DwmSetWindowAttribute(hwnd, DWMWA_USE_HOSTBACKDROPBRUSH,
            &enabled as *const _ as _, std::mem::size_of_val(&enabled) as u32)?; }
        let compositor = Compositor::new()?;
        let interop: ICompositorDesktopInterop = compositor.cast()?;
        let target = unsafe { interop.CreateDesktopWindowTarget(hwnd, false)? };
        let root = compositor.CreateContainerVisual()?;
        target.SetRoot(&root)?;
        Ok(Self { compositor, target, root, _queue: queue, surfaces: Vec::new() })
    }

    pub fn sync(&mut self, surfaces: &[BackdropSurface]) -> Result<()> {
        if self.surfaces == surfaces { return Ok(()); }
        // Build the replacement before attaching it, so errors cannot leave partial surfaces.
        let next = self.compositor.CreateContainerVisual()?;
        for surface in surfaces {
            let visual = self.compositor.CreateSpriteVisual()?;
            let mut size = visual.Size()?;
            size.X = surface.width; size.Y = surface.height;
            visual.SetSize(size)?;
            let mut offset = visual.Offset()?;
            offset.X = surface.x; offset.Y = surface.y;
            visual.SetOffset(offset)?;
            let source_name = HSTRING::from("Backdrop");
            let effect: IGraphicsEffect = BlurEffect {
                amount: surface.blur,
                source: CompositionEffectSourceParameter::Create(&source_name)?.cast()?,
            }.into();
            let brush = self.compositor.CreateEffectFactory(&effect)?.CreateBrush()?;
            brush.SetSourceParameter(&source_name, &self.compositor.CreateHostBackdropBrush()?)?;
            visual.SetBrush(&brush)?;
            let clip = if surface.slot == 0 {
                let source: IGeometrySource2D = GeometrySource(notch_geometry(surface)?).into();
                let path = CompositionPath::Create(&source)?;
                let geometry = self.compositor.CreatePathGeometryWithPath(&path)?;
                self.compositor.CreateGeometricClipWithGeometry(&geometry)?
            } else {
                let geometry = self.compositor.CreateRoundedRectangleGeometry()?;
                geometry.SetSize(size)?;
                let mut radius = geometry.CornerRadius()?;
                radius.X = surface.radius.min(surface.width / 2.).min(surface.height / 2.);
                radius.Y = radius.X;
                geometry.SetCornerRadius(radius)?;
                self.compositor.CreateGeometricClipWithGeometry(&geometry)?
            };
            visual.SetClip(&clip)?;
            next.Children()?.InsertAtTop(&visual)?;
        }
        self.target.SetRoot(&next)?;
        self.root = next;
        self.surfaces = surfaces.to_vec();
        Ok(())
    }
}

fn notch_geometry(surface: &BackdropSurface) -> Result<ID2D1Geometry> {
    let edge = surface.edge.as_deref().unwrap_or("right");
    let vertical = matches!(edge, "left" | "right");
    let depth = if vertical { surface.width } else { surface.height };
    let length = if vertical { surface.height } else { surface.width };
    let radius = depth / 2.;
    let along = if surface.radius > 0. { surface.radius } else { radius };
    let point = |x, y| {
        let (x, y) = match edge {
            "left" => (depth - x, y), "bottom" => (y, x), "top" => (y, depth - x), _ => (x, y),
        };
        let mut p = D2D1_ARC_SEGMENT::default().point;
        p.X = x; p.Y = y; p
    };
    unsafe {
        let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_MULTI_THREADED, None)?;
        let geometry = factory.CreatePathGeometry()?;
        let sink = geometry.Open()?;
        sink.BeginFigure(point(depth, 0.), D2D1_FIGURE_BEGIN_FILLED);
        for (x, y, clockwise, line) in [
            (radius, along, true, false), (0., along * 2., false, false),
            (0., length - along * 2., false, true), (radius, length - along, false, false),
            (depth, length, true, false),
        ] {
            if line { sink.AddLine(point(x, y)); continue; }
            let reflected = matches!(edge, "left" | "bottom");
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: point(x, y), size: if vertical { D2D_SIZE_F { width: radius, height: along } } else { D2D_SIZE_F { width: along, height: radius } },
                rotationAngle: 0., arcSize: D2D1_ARC_SIZE_SMALL,
                sweepDirection: if clockwise != reflected { D2D1_SWEEP_DIRECTION_CLOCKWISE } else { D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE },
            });
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        geometry.cast()
    }
}
