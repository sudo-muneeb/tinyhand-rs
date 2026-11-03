#!/usr/bin/env python3
"""Exercise TinyHand in a private Xvfb, D-Bus and Cinnamon session.
Never sends input to the user's desktop. Requires Python gi, pyatspi, Pillow,
Xvfb, Cinnamon, dbus-daemon, xdotool, xprop, xev, xmessage and ImageMagick.
"""
import argparse,ctypes,json,os,re,select,signal,subprocess,tempfile,time
from pathlib import Path
from PIL import Image,ImageChops,ImageDraw

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cycles',type=int,default=3)
    parser.add_argument('--binary',type=Path,default=Path(__file__).resolve().parents[1]/'target/release/tinyhand-rs')
    args=parser.parse_args();binary=args.binary.resolve()
    assert args.cycles>0 and binary.is_file()
    output=Path(tempfile.mkdtemp(prefix='tinyhand-cinnamon-'));print(f'Logs: {output}',flush=True)
    env=os.environ.copy()
    for key in ['DBUS_SESSION_BUS_ADDRESS','AT_SPI_BUS_ADDRESS','SESSION_MANAGER','WAYLAND_DISPLAY','GDK_SCALE','QT_SCALE_FACTOR']:env.pop(key,None)
    for key in ['XDG_CONFIG_HOME','XDG_CACHE_HOME','XDG_DATA_HOME','XDG_RUNTIME_DIR']:
        directory=output/key;directory.mkdir(mode=0o700);env[key]=str(directory)
    env.update(GSETTINGS_BACKEND='memory',LIBGL_ALWAYS_SOFTWARE='1',XDG_SESSION_TYPE='x11',XDG_CURRENT_DESKTOP='X-Cinnamon',TINYHAND_TEST_SESSION=str(output),GTK_MODULES='gail:atk-bridge',NO_AT_BRIDGE='0',TINYHAND_TRACE_INPUT='1')
    alsa=output/'asound.conf';alsa.write_text('pcm.!default { type null }\n');env['ALSA_CONFIG_PATH']=str(alsa)
    processes=[]
    def start(command,name,**kw):
        with (output/f'{name}.log').open('w') as log:p=subprocess.Popen(command,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True,**kw)
        processes.append(p);return p
    def run(*cmd,check=True):
        p=subprocess.run(cmd,env=env,text=True,capture_output=True,timeout=15)
        if check and p.returncode:raise RuntimeError(f'{cmd}: {p.stderr}')
        return p
    def wait(predicate,message,timeout=12):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            if predicate():return
            time.sleep(.1)
        raise AssertionError(message)
    def visible():return run('xdotool','search','--onlyvisible','--name','^TinyHand overlay$',check=False).returncode==0
    def command(flag):run(str(binary),flag)
    def click(x=100,y=100,button=1):run('xdotool','mousemove',str(x),str(y),'click',str(button));time.sleep(.15)
    def screenshot(name):
        path=output/f'{name}.png';run('import','-window','root',str(path));return Image.open(path).convert('RGB')
    def probe_clicks():return (output/'input.log').read_text().count('ButtonPress event')
    try:
        rd,wr=os.pipe();start(['Xvfb','-displayfd',str(wr),'-screen','0','1280x800x24','-nolisten','tcp','-noreset'],'xvfb',pass_fds=(wr,));os.close(wr)
        assert select.select([rd],[],[],10)[0];env['DISPLAY']=':'+os.read(rd,64).decode().strip();os.close(rd)
        bus=run('dbus-daemon','--session','--fork','--print-address=1','--print-pid=1').stdout.splitlines();env['DBUS_SESSION_BUS_ADDRESS']=bus[0];bus_pid=int(bus[1])
        os.environ.update(env)
        wm=start(['cinnamon','--sm-disable','--x11'],'cinnamon')
        wait(lambda:'window id #' in run('xprop','-root','_NET_SUPPORTING_WM_CHECK').stdout,'Cinnamon did not start',30);time.sleep(4)
        import pyatspi
        def walk(node):
            yield node
            for child in node:
                if child is not None:yield from walk(child)
        def find(name,role=None):
            try:
                for app in pyatspi.Registry.getDesktop(0):
                    if app.name=='tinyhand-rs':
                        for node in walk(app):
                            if node.name==name and (role is None or node.getRoleName()==role):return node
            except Exception:pass
        def action(name):
            wait(lambda:find(name) is not None,f'Missing accessible control: {name}')
            node=find(name);actions=node.queryAction();assert actions.nActions>0,name;assert actions.doAction(0),name;time.sleep(.15)
        cfg=Path(env['XDG_CONFIG_HOME'])/'tinyhand-rs/settings.conf';cfg.parent.mkdir();cfg.write_text('welcomed=true\nsound=false\nstyle=laser\ncolor=0\nmatch_color=false\n')
        probe=start(['stdbuf','-oL','xev','-name','TinyHand input probe','-geometry','240x140+20+20','-event','button'],'input');time.sleep(1)
        wait(lambda:run('xdotool','search','--onlyvisible','--name','^TinyHand input probe$',check=False).returncode==0,'Input probe did not map')
        wait(lambda:(click() or probe_clicks()>0),'The baseline input probe is not receiving clicks')
        probe_id=run('xdotool','search','--name','^TinyHand input probe$').stdout.splitlines()[0]
        probe_geometry=run('xdotool','getwindowgeometry','--shell',probe_id).stdout
        def modifiers_up():
            x=ctypes.CDLL('libX11.so.6');x.XOpenDisplay.restype=ctypes.c_void_p;x.XOpenDisplay.argtypes=[ctypes.c_char_p]
            x.XQueryKeymap.argtypes=[ctypes.c_void_p,ctypes.c_void_p];x.XKeysymToKeycode.argtypes=[ctypes.c_void_p,ctypes.c_ulong];x.XKeysymToKeycode.restype=ctypes.c_ubyte;x.XCloseDisplay.argtypes=[ctypes.c_void_p]
            display=x.XOpenDisplay(env['DISPLAY'].encode());keys=(ctypes.c_ubyte*32)();x.XQueryKeymap(display,keys)
            result=all(not keys[(key:=x.XKeysymToKeycode(display,sym))//8]&(1<<(key%8)) for sym in [0xffe9,0xffea,0xffe3,0xffe4,0xffeb,0xffec]);x.XCloseDisplay(display);return result
        def device_alt(pressed):
            # Use the separate Xvfb keyboard device as well as the XTEST
            # keyboard, so modifier suppression crosses device boundaries.
            class DeviceInfo(ctypes.Structure):
                _fields_=[('id',ctypes.c_int),('name',ctypes.c_char_p),('use',ctypes.c_int),('attachment',ctypes.c_int),('enabled',ctypes.c_int),('classes_count',ctypes.c_int),('classes',ctypes.c_void_p)]
            x=ctypes.CDLL('libX11.so.6');xi=ctypes.CDLL('libXi.so.6');xt=ctypes.CDLL('libXtst.so.6')
            x.XOpenDisplay.restype=ctypes.c_void_p;x.XOpenDisplay.argtypes=[ctypes.c_char_p];x.XCloseDisplay.argtypes=[ctypes.c_void_p];x.XSync.argtypes=[ctypes.c_void_p,ctypes.c_int]
            x.XKeysymToKeycode.argtypes=[ctypes.c_void_p,ctypes.c_ulong];x.XKeysymToKeycode.restype=ctypes.c_ubyte
            xi.XIQueryDevice.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.POINTER(ctypes.c_int)];xi.XIQueryDevice.restype=ctypes.POINTER(DeviceInfo);xi.XIFreeDeviceInfo.argtypes=[ctypes.POINTER(DeviceInfo)]
            xi.XOpenDevice.argtypes=[ctypes.c_void_p,ctypes.c_ulong];xi.XOpenDevice.restype=ctypes.c_void_p;xi.XCloseDevice.argtypes=[ctypes.c_void_p,ctypes.c_void_p]
            xt.XTestFakeDeviceKeyEvent.argtypes=[ctypes.c_void_p,ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_void_p,ctypes.c_int,ctypes.c_ulong]
            display=x.XOpenDisplay(env['DISPLAY'].encode());n=ctypes.c_int();devices=xi.XIQueryDevice(display,0,ctypes.byref(n));ids=[devices[i].id for i in range(n.value) if devices[i].use==4 and devices[i].enabled and b'XTEST' not in devices[i].name];xi.XIFreeDeviceInfo(devices);assert ids,'No independent keyboard device'
            device=xi.XOpenDevice(display,ids[0]);assert device;assert xt.XTestFakeDeviceKeyEvent(display,device,x.XKeysymToKeycode(display,0xffe9),int(pressed),None,0,0);x.XSync(display,0);xi.XCloseDevice(display,device);x.XCloseDisplay(display)
        rss=[]
        for cycle in range(args.cycles):
            app=start([str(binary),'--settings' if cycle==0 else '--present'],f'app-{cycle+1}')
            if cycle==0:
                wait(lambda:find('Start pointer') is not None,'Native settings did not open',25)
                assert not visible(),'Startup captured the desktop before activation'
                for tab in ['General','Pointers','Appearance','Drawing','Sound','About']:
                    action(tab);screenshot(f'settings-{tab.lower()}')
                action('Sound')
                for kind,title in enumerate(['Thock','Click','Pop','Wood block','Boop','Squeak'],1):
                    action(title);wait(lambda:f'sound_kind={kind}\n' in cfg.read_text(),f'{title} did not persist')
                    assert find(title).getState().contains(pyatspi.STATE_CHECKED),f'{title} selection did not update'
                action('Off');wait(lambda:'sound=false' in cfg.read_text(),'Sound off did not persist')
                action('General')
                shortcut=find('Turn the pointer on or off','text');assert shortcut.queryComponent().grabFocus();time.sleep(.15)
                run('xdotool','key','ctrl+alt+x');time.sleep(.3)
                assert 'toggle=Control+Alt+h' in cfg.read_text() and 'clear=Control+Alt+x' in cfg.read_text(),'Conflicting shortcut did not roll back'
                recorder=find('Turn the pointer on or off','text');assert recorder.queryComponent().grabFocus();time.sleep(.1);run('xdotool','key','Tab');time.sleep(.1)
                assert 'Press' not in recorder.queryText().getText(0,-1),'Leaving the shortcut recorder did not cancel recording'
                run('xdotool','key','Escape')
                action('Appearance');scale=find('Size','slider').queryValue();scale.currentValue=.16;wait(lambda:'size=0.16' in cfg.read_text(),'Size did not persist')
                action('Drawing');action('Match');wait(lambda:'match_color=true' in cfg.read_text(),'Match colour did not persist')
                # Selecting via the native pointer library also exercises persistence.
                action('Pointers');action('Pin blue');action('Pin mint');action('Pin lemon');action('Replace lavender')
                wait(lambda:'lemon' in cfg.read_text().split('pinned=')[1].splitlines()[0] and 'lavender' not in cfg.read_text().split('pinned=')[1].splitlines()[0],'Full-toolbar replacement failed')
                action('Reset toolbar')
                action('Use coral');wait(lambda:'style=coral' in cfg.read_text(),'Pointer selection did not persist')
                action('Appearance');preview=find('Pointer preview');bounds=preview.queryComponent().getExtents(pyatspi.DESKTOP_COORDS);box=(bounds.x,bounds.y,bounds.x+bounds.width,bounds.y+bounds.height)
                first=screenshot('appearance-hand').crop(box);find('Size','slider').queryValue().currentValue=.18;time.sleep(.2);second=screenshot('appearance-hand-adjusted').crop(box)
                assert ImageChops.difference(first,second).getbbox() is not None,'The appearance preview did not update with the slider'
                find('Size','slider').queryValue().currentValue=.16
                action('Pointers');action('Use laser')
                action('Pin lavender');wait(lambda:'pinned=glove,coral,plush,robot,laser' in cfg.read_text(),'Pin removal failed')
                action('Pin lavender');wait(lambda:'lavender' in cfg.read_text().split('pinned=')[1].splitlines()[0],'Pin addition failed')
                # Validate and compose hand-only artwork, then import through the native chooser.
                fixture=output/'hand.png';im=Image.new('RGBA',(512,768));draw=ImageDraw.Draw(im)
                draw.polygon([(245,70),(300,70),(300,320),(390,340),(390,540),(330,575),(330,690),(200,690),(200,575),(130,540),(130,340),(245,320)],fill=(235,70,140,255));im.save(fixture)
                composed=output/'composed.png';run(str(binary),'--make-hand',str(fixture),str(composed));assert Image.open(composed).size==(1024,1536)
                invalid=output/'opaque.png';Image.new('RGB',(512,768),'white').save(invalid);assert run(str(binary),'--make-hand',str(invalid),str(output/'bad.png'),check=False).returncode==1
                action('Make your own hand…');action('3. Drop an image here, or click to choose it')
                wait(lambda:run('xdotool','search','--onlyvisible','--name','Choose your TinyHand image',check=False).returncode==0,'Native chooser did not open')
                run('xdotool','key','ctrl+l');run('xdotool','type','--clearmodifiers',str(fixture));run('xdotool','key','Return');time.sleep(.4);run('xdotool','key','Return')
                wait(lambda:find('Add hand') is not None and find('Add hand').getState().contains(pyatspi.STATE_SENSITIVE),'Valid custom hand was rejected',20)
                name=find('Name','text');name.queryEditableText().setTextContents('Test hand');action('Add hand')
                library=Path(env['XDG_DATA_HOME'])/'tinyhand-rs/hands/hands.json';wait(lambda:library.exists() and 'Test hand' in library.read_text(),'Custom hand did not persist')
                original=fixture;original.unlink();assert list(library.parent.glob('custom-*.png')),'The artwork was not copied to the managed library'
                assert 'custom_enabled=true' in cfg.read_text()
                screenshot('custom-hand-library');action('Delete');wait(lambda:json.loads(library.read_text())==[],'Custom hand delete failed')
                action('Start pointer')
            wait(visible,'Overlay did not appear');time.sleep(.2)
            # Single-instance CLI must control, not launch a second capture loop.
            before=probe_clicks();click();assert probe_clicks()==before,'Presentation leaked a click'
            # Interaction strips Alt from the forwarded click, then regrabs on release.
            run('xdotool','keydown','Alt_L','mousemove','100','100','mousedown','1');time.sleep(.2)
            run('xdotool','mouseup','1','keyup','Alt_L');wait(lambda:probe_clicks()>before,'Modifier interaction did not forward the click')
            text=(output/'input.log').read_text();last=text.rsplit('ButtonPress event',1)[1];state=int(re.search(r'state (0x[0-9a-f]+)',last).group(1),16);assert state&8==0,f'Interaction leaked Alt: {state}'
            time.sleep(.2);before=probe_clicks();click();assert probe_clicks()==before,'Pointer was not recaptured after interaction'
            if cycle==0:
                before=probe_clicks();device_alt(True);run('xdotool','mousemove','100','100','mousedown','1');time.sleep(.15)
                run('xdotool','mousemove','150','115','mouseup','1');device_alt(False);time.sleep(.2)
                wait(lambda:probe_clicks()>before,'Independent keyboard interaction did not forward the click');wait(modifiers_up,'Independent keyboard modifier remained pressed')
                last=(output/'input.log').read_text().rsplit('ButtonPress event',1)[1];state=int(re.search(r'state (0x[0-9a-f]+)',last).group(1),16);assert state&8==0,'Independent keyboard Alt leaked into the click'
                released=(output/'input.log').read_text().rsplit('ButtonRelease event',1)[1];release_state=int(re.search(r'state (0x[0-9a-f]+)',released).group(1),16);assert release_state&8==0,'Independent keyboard Alt leaked into the release'
                assert run('xdotool','getwindowgeometry','--shell',probe_id).stdout==probe_geometry,'Independent modifier drag moved the window'
            # Wheel replay must release the synthetic wheel button, including
            # devices which deliver their physical release before replay starts.
            before=probe_clicks();run('xdotool','keydown','Alt_L','mousemove','100','100','click','4','keyup','Alt_L');wait(lambda:probe_clicks()>before,'Interaction scrolling was not forwarded');time.sleep(.2)
            before=probe_clicks();click();assert probe_clicks()==before,'Wheel replay left capture released'
            # Releasing the interaction modifier before the mouse must not leave it stuck.
            before=probe_clicks();run('xdotool','keydown','Alt_L','mousemove','100','100','mousedown','1','mousemove','160','120','keyup','Alt_L');time.sleep(.1)
            run('xdotool','mouseup','1');wait(lambda:probe_clicks()>before,'Interaction drag was not forwarded');time.sleep(.2)
            wait(modifiers_up,'An interaction modifier remained logically pressed')
            assert run('xdotool','getwindowgeometry','--shell',probe_id).stdout==probe_geometry,'Interaction accidentally moved the underlying window'
            before=probe_clicks();click();assert probe_clicks()==before,'Capture was not restored after early modifier release'
            run('xdotool','mousemove','300','220','mousedown','1','mousemove','700','220');time.sleep(.1)
            def drawing_ready():
                pixel=screenshot(f'drawing-{cycle}').getpixel((400,220));return max(pixel)-min(pixel)>60
            wait(drawing_ready,'The compositor did not publish the drawn line',3)
            run('xdotool','key','ctrl+alt+x')
            wait(lambda:screenshot('cleared-while-held').getpixel((400,220))==(0,0,0),'Clear shortcut failed while the mouse was held',3)
            run('xdotool','mouseup','1');time.sleep(.2)
            # Right-button drags animate/tap but must never create ink.
            command('--clear');time.sleep(.2);baseline=screenshot('before-right-drag')
            run('xdotool','mousemove','300','280','mousedown','3','mousemove','700','280','mouseup','3');time.sleep(.3)
            right=screenshot('after-right-drag');assert right.getpixel((400,280))==baseline.getpixel((400,280)),'Right drag drew ink'
            if cycle in (0,1):
                # Measure both laser and hand fades after settling pointer motion.
                run('xdotool','mousemove','1100','100');time.sleep(1.1)
                trace=output/f'app-{cycle+1}.log';offset=trace.stat().st_size
                work=list(map(int,re.findall(r'-?\d+',run('xprop','-root','_NET_WORKAREA').stdout.split('=',1)[1])));bottom=work[1]+work[3]
                run('xdotool','mousemove','640',str(bottom-35));time.sleep(.45)
                screenshot('toolbar-polished')
                run('xdotool','mousemove','1100','100');time.sleep(1.3)
                records=re.findall(r'took (\d+)ms, strokes=\d+, toolbar=([0-9.]+), time_us=(\d+)',trace.read_text()[offset:])
                if records:
                    frames=[(int(us),float(alpha),int(cost)) for cost,alpha,us in records]
                    rising=[f for i,f in enumerate(frames[1:],1) if 0<f[1]<1 and f[1]>frames[i-1][1]]
                    falling=[f for i,f in enumerate(frames[1:],1) if 0<f[1]<1 and f[1]<frames[i-1][1]]
                    intervals=[b[0]-a[0] for group in [rising,falling] for a,b in zip(group,group[1:])]
                    assert len(rising)>=6 and len(falling)>=15,f'Too few fade frames: {len(rising)}, {len(falling)}'
                    intervals.sort();p95=intervals[int((len(intervals)-1)*.95)]/1000
                    assert p95<30,f'Toolbar frame pacing regressed: p95 {p95:.1f}ms'
                    metrics={'pointer_style':cfg.read_text().split('style=')[1].splitlines()[0],'reveal_frames':len(rising),'hide_frames':len(falling),'median_frame_ms':intervals[len(intervals)//2]/1000,'p95_frame_ms':p95,'max_render_ms':max(f[2] for f in rising+falling)}
                    (output/f'toolbar-performance-{cycle}.json').write_text(json.dumps(metrics,indent=2));print('Toolbar timing: '+json.dumps(metrics),flush=True)
                else:
                    print('Baseline fade frames: '+str(trace.read_text()[offset:].count('frame at ')),flush=True)
            # Modern D-Bus menu must expose exact protocol signatures and actions.
            name=f'org.kde.StatusNotifierItem-{app.pid}-1'
            layout=run('gdbus','call','--session','--dest',name,'--object-path','/Menu','--method','com.canonical.dbusmenu.GetLayout','--','0','-1','[]').stdout
            assert 'Start pointer' not in layout and 'Stop pointer' in layout and 'Settings' in layout,layout
            if cycle==0:
                # Toolbar drag/reorder and drag-out removal use the actual pointer path.
                work=list(map(int,re.findall(r'-?\d+',run('xprop','-root','_NET_WORKAREA').stdout.split('=',1)[1])));bottom=work[1]+work[3]
                pins=cfg.read_text().split('pinned=')[1].splitlines()[0].split(',')
                width=(len(pins)+5)*44+12;bx=(1280-width)/2;by=bottom-62
                run('xdotool','mousemove',str(round(bx+28)),str(round(by+27)));time.sleep(.3)
                run('xdotool','mousedown','1','mousemove',str(round(bx+116)),str(round(by+27)),'mouseup','1')
                expected=pins.copy();expected.insert(2,expected.pop(0));wait(lambda:'pinned='+','.join(expected) in cfg.read_text(),'Toolbar drag reorder failed')
                run('xdotool','mousemove',str(round(bx+28)),str(round(by+27)),'mousedown','1','mousemove','640','580','mouseup','1')
                expected.pop(0);wait(lambda:'pinned='+','.join(expected) in cfg.read_text(),'Toolbar drag-out removal failed');assert visible(),'Toolbar drag opened settings'
                selected=cfg.read_text().split('style=')[1].splitlines()[0];count=len(expected)+int(selected not in expected)
                width=(count+5)*44+12;bx=(1280-width)/2
                run('xdotool','mousemove',str(round(bx+28+count*44)),str(round(by+27)));time.sleep(.3)
                run('xdotool','click','1');time.sleep(.2);click(round(bx+223),round(by-35))
                wait(lambda:'color=4' in cfg.read_text() and 'match_color=false' in cfg.read_text(),'Quick ink palette failed')
                command('--clear');run('xdotool','mousemove','300','300','mousedown','1','mousemove','700','300');time.sleep(.2)
                def blue_ready():
                    actual=screenshot('quick-blue-ink').getpixel((400,300));return max(abs(a-b) for a,b in zip(actual,(10,132,255)))<=2
                wait(blue_ready,'The quick palette did not render blue ink',3)
                run('xdotool','mouseup','1');time.sleep(3.9);assert screenshot('faded-ink').getpixel((400,300))==(0,0,0),'Ink did not finish fading'
            command('--settings');wait(lambda:not visible(),'Opening settings did not release input')
            wait(lambda:find('Start pointer') is not None,'Native settings did not become ready')
            run('xdotool','windowactivate','--sync',probe_id)
            before=probe_clicks();wait(lambda:(click() or probe_clicks()>before),'Settings kept the pointer grab',3)
            command('--present');wait(visible,'Remote start failed')
            run('xdotool','key','Escape');wait(lambda:not visible(),'Escape failed')
            run('xdotool','key','ctrl+alt+h');wait(visible,'Global shortcut failed')
            command('--clear');run('xdotool','mousemove','1100','100');time.sleep(4)
            def ticks():return sum(map(int,Path(f'/proc/{app.pid}/stat').read_text().split()[13:15]))
            t=ticks();time.sleep(2);cpu=(ticks()-t)/os.sysconf('SC_CLK_TCK')/2*100;assert cpu<5,f'Idle CPU {cpu:.1f}%';print(f'Cycle {cycle+1} idle CPU {cpu:.1f}%',flush=True)
            rss.append(int(Path(f'/proc/{app.pid}/status').read_text().split('VmRSS:')[1].split()[0]))
            command('--quit');assert app.wait(timeout=8)==0;wait(lambda:not visible(),'Overlay remained after exit')
            before=probe_clicks();click();assert probe_clicks()>before,'Shutdown left input captured'
            for _ in range(2):run('xmessage','-timeout','1','-buttons','OK','Teardown check');assert wm.poll() is None,'Cinnamon crashed after teardown'
            log=(output/f'app-{cycle+1}.log').read_text();assert 'X11 error' not in log and 'CRITICAL' not in log,log
            print(f'Cycle {cycle+1}/{args.cycles}: native UI, capture, interaction, ink, SNI, shortcuts, shutdown passed',flush=True)
        time.sleep(5);assert wm.poll() is None
        log=(output/'cinnamon.log').read_text();assert 'Adding systray: unknown' not in log and 'has been already disposed' not in log
        result={'result':'PASS','cycles':args.cycles,'max_app_rss_kib':max(rss),'logs':str(output)};(output/'result.json').write_text(json.dumps(result,indent=2));print(json.dumps(result),flush=True)
    finally:
        for p in reversed(processes):
            if p.poll() is None:os.killpg(p.pid,signal.SIGTERM)
        for p in processes:
            try:p.wait(timeout=5)
            except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=5)
        marker=f'TINYHAND_TEST_SESSION={output}'.encode()
        for proc in Path('/proc').iterdir():
            if proc.name.isdecimal() and int(proc.name)!=os.getpid():
                try:
                    if marker in (proc/'environ').read_bytes().split(b'\0'):os.kill(int(proc.name),signal.SIGTERM)
                except (PermissionError,FileNotFoundError,ProcessLookupError):pass
        print(f'Isolated session stopped. Logs: {output}',flush=True)
if __name__=='__main__':main()
