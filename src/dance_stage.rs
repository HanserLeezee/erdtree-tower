pub fn position(slot:usize,count:usize)->[f32;3]{
    let count=count.clamp(1,8);let slot=slot.min(count-1);let columns=count.min(4);
    let row=slot/columns;let row_count=(count-row*columns).min(columns);
    [(slot%columns) as f32*2.8-(row_count-1) as f32*1.4,0.0,4.0+row as f32*3.0]
}
#[derive(Clone,Copy,Debug)]
pub struct Shot {pub yaw:f32,pub distance:f32,pub height:f32,pub target_height:f32,pub group:bool}
pub fn shot(seconds:f64,beat:f64,count:usize)->Shot{
    let phase=(seconds/(beat*16.0)).rem_euclid(4.0);let group=count>1&&phase>=3.0;
    if (1.0..1.25).contains(&phase)||(2.5..2.75).contains(&phase){
        return Shot{yaw:if phase<2.0{0.0}else{0.25},distance:2.2,height:1.4,target_height:1.25,group:false};
    }
    let angle=match phase as u32{0=>0.0,1=>-0.42,2=>0.42,_=>0.0};
    Shot{yaw:angle,distance:if group{11.0}else{4.5},height:if group{4.0}else{1.6},target_height:1.0,group}
}
#[cfg(test)]mod tests{
    #[test]fn all_players_fit_and_never_overlap(){for count in 1..=8{for i in 0..count{let a=super::position(i,count);assert!(a[0].abs()<=4.3&&a[2]<=7.0);for j in 0..i{let b=super::position(j,count);assert!((a[0]-b[0]).hypot(a[2]-b[2])>=2.7);}}}}
    #[test]fn shots_keep_full_body_and_add_group_view(){assert!(super::shot(0.0,0.5,1).distance>=4.0);assert!(super::shot(25.0,0.5,8).group);assert!(!super::shot(25.0,0.5,1).group);}
    #[test]fn closeups_are_brief_and_return_to_full_body(){
        for count in [1,8]{
            for second in [8.0,9.9,20.0,21.9]{let s=super::shot(second,0.5,count);assert!((2.0..2.5).contains(&s.distance));assert!(!s.group);assert!(s.target_height>=1.2);}
            for second in [7.9,10.0,19.9,22.0]{assert!(super::shot(second,0.5,count).distance>=4.0);}
        }
    }
}
