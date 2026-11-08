target triple = "x86_64-pc-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
; ModuleID = 'tests.slice16'
source_filename = "tests.slice16"

@ignite.empty = private constant [8 x i8] zeroinitializer, align 8
@.str.2 = private unnamed_addr constant [1 x i8] c"*", align 1
@.str.1 = private unnamed_addr constant [1 x i8] c"+", align 1
@.str.0 = private unnamed_addr constant [27 x i8] c"tests/golden/slice16.ignite", align 1

declare void @ignite_rt_init(i32, ptr)
declare ptr @ignite_rt_alloc(i64, i64)
declare i8 @ignite_rt_spawn(ptr, ptr)
declare void @ignite_rt_spawn_ready()
declare void @ignite_rt_task_end()
declare i64 @ignite_rt_arena_mark()
declare void @ignite_rt_arena_release(i64)
declare i8 @ignite_rt_arena_due(i64, i64)
declare void @ignite_rt_arena_scratch(i8)
declare void @ignite_rt_arena_scratch_clear()
declare ptr @ignite_rt_arena_state() nounwind willreturn speculatable memory(none)
declare i32 @ignite_rt_args_port(ptr, ptr, ptr)
declare void @ignite_rt_exit(i32) noreturn
declare void @ignite_rt_fail(ptr) cold noreturn
declare void @ignite_rt_log(ptr)
declare i32 @ignite_rt_tcp_bind(ptr, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_accept(i64, ptr, ptr)
declare i64 @ignite_rt_tcp_local_port(i64)
declare i32 @ignite_rt_tcp_connect(ptr, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_read(i64, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_write_all(i64, ptr, ptr)
declare i32 @ignite_rt_tcp_close(i64, ptr)
declare void @ignite_rt_tcp_release(i64)
declare i32 @ignite_rt_stdin_read(i64, ptr, ptr)
declare i32 @ignite_rt_stdin_is_terminal(ptr, ptr)
declare i32 @ignite_rt_stdin_set_raw(i8, ptr)
declare i32 @ignite_rt_stdout_write_all(ptr, ptr)
declare i32 @ignite_rt_stderr_write_all(ptr, ptr)
declare void @ignite_rt_io_error_new(ptr, ptr)
declare void @ignite_rt_text_from_bytes_lossy(ptr, ptr)
declare void @ignite_rt_text_trim(ptr, ptr)
declare void @ignite_rt_text_concat(ptr, ptr, ptr)
declare i8 @ignite_rt_text_eq(ptr, ptr)
declare i8 @ignite_rt_text_starts_with(ptr, ptr)
declare void @ignite_rt_text_join(ptr, ptr, ptr)
declare void @ignite_rt_int_to_text(ptr, i64)
declare i8 @ignite_rt_int_parse(ptr, ptr)
declare i32 @ignite_rt_tcp_set_idle_timeout(i64, i64, ptr)
declare ptr @ignite_rt_store_new(ptr, ptr, ptr, i64, i64)
declare i64 @ignite_rt_store_next_id(ptr)
declare void @ignite_rt_store_put(ptr, i64, ptr)
declare i8 @ignite_rt_store_get(ptr, i64, ptr)
declare i8 @ignite_rt_store_remove(ptr, i64, ptr)
declare void @ignite_rt_store_values(ptr, ptr)
declare i8 @ignite_rt_store_update(ptr, i64, ptr, ptr, ptr)
declare ptr @malloc(i64)
declare void @free(ptr)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)
declare { i8, i1 } @llvm.uadd.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.usub.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.umul.with.overflow.i8(i8, i8)
declare void @"ignite_rt_fail_arith"(ptr, ptr, i64, i64, i64, i64) cold noreturn
declare void @"ignite_rt_fail_index"(ptr, i64, i64, i64, i64) cold noreturn

define internal void @ignite.reserve(ptr %list, i64 %extra, i64 %size, i64 %align) alwaysinline {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %need = add i64 %len, %extra
  %fits = icmp sle i64 %need, %cap
  br i1 %fits, label %done, label %grow
grow:
  call void @ignite.grow(ptr %list, i64 %need, i64 %size, i64 %align)
  br label %done
done:
  ret void
}

define internal void @ignite.grow(ptr %list, i64 %need, i64 %size, i64 %align) noinline cold {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %double = mul i64 %cap, 2
  %big = icmp sgt i64 %double, %need
  %c1 = select i1 %big, i64 %double, i64 %need
  %small = icmp slt i64 %c1, 4
  %newcap = select i1 %small, i64 4, i64 %c1
  %bytes = mul i64 %newcap, %size
  %new = call ptr @ignite.alloc(i64 %bytes, i64 %align)
  %old = load ptr, ptr %list, align 8
  %used = mul i64 %len, %size
  call void @llvm.memcpy.p0.p0.i64(ptr %new, ptr %old, i64 %used, i1 false)
  store ptr %new, ptr %list, align 8
  store i64 %newcap, ptr %capp, align 8
  ret void
}

define internal ptr @ignite.alloc(i64 %size, i64 %align) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %limp = getelementptr inbounds i8, ptr %st, i64 8
  %lim = load ptr, ptr %limp, align 8
  %ci = ptrtoint ptr %cur to i64
  %li = ptrtoint ptr %lim to i64
  %am1 = sub i64 %align, 1
  %up = add i64 %ci, %am1
  %mask = sub i64 0, %align
  %start = and i64 %up, %mask
  %end = add i64 %start, %size
  %small = icmp ult i64 %size, 65536
  %fits = icmp ule i64 %end, %li
  %live = icmp ne i64 %li, 0
  %fit = and i1 %fits, %live
  %ok = and i1 %small, %fit
  br i1 %ok, label %fast, label %slow
fast:
  %skip = sub i64 %start, %ci
  %p = getelementptr i8, ptr %cur, i64 %skip
  %next = getelementptr i8, ptr %p, i64 %size
  store ptr %next, ptr %st, align 8
  ret ptr %p
slow:
  %q = call ptr @ignite_rt_alloc(i64 %size, i64 %align)
  ret ptr %q
}

define internal i64 @ignite.arena_mark() alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  ret i64 %pos
}

define internal i1 @ignite.arena_due(i64 %mark, i64 %kept) alwaysinline {
entry:
  %pos = call i64 @ignite.arena_mark()
  %above = sub i64 %pos, %mark
  %four = shl i64 %kept, 2
  %big = icmp sgt i64 %four, 65536
  %limit = select i1 %big, i64 %four, i64 65536
  %due = icmp sgt i64 %above, %limit
  ret i1 %due
}

define internal void @ignite.arena_release(i64 %mark) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  %nothing = icmp sge i64 %mark, %pos
  br i1 %nothing, label %done, label %down
down:
  %basep = getelementptr inbounds i8, ptr %st, i64 24
  %base = load ptr, ptr %basep, align 8
  %bi = ptrtoint ptr %base to i64
  %bpos = add i64 %bi, %off
  %inside = icmp sge i64 %mark, %bpos
  br i1 %inside, label %chunk, label %slow
chunk:
  %back = sub i64 %mark, %bpos
  %new = getelementptr i8, ptr %base, i64 %back
  store ptr %new, ptr %st, align 8
  br label %done
slow:
  call void @ignite_rt_arena_release(i64 %mark)
  br label %done
done:
  ret void
}

define void @"tests.slice16.step"(ptr %sret, ptr %p0, i64 %p1) !dbg !4 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t2 = alloca [32 x i8], align 8
  %t17 = alloca [24 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  store i64 %p1, ptr %t1, align 8, !dbg !5
  call void @llvm.memcpy.p0.p0.i64(ptr %t2, ptr %p0, i64 32, i1 false), !dbg !6
  %t3 = load i64, ptr %t1, align 8, !dbg !9
  call void @ignite.reserve(ptr %t2, i64 1, i64 8, i64 8), !dbg !8
  %t4 = getelementptr inbounds i8, ptr %t2, i64 8, !dbg !8
  %t5 = load i64, ptr %t4, align 8, !dbg !8
  %t6 = load ptr, ptr %t2, align 8, !dbg !8
  %t7 = mul i64 %t5, 8, !dbg !8
  %t8 = getelementptr inbounds i8, ptr %t6, i64 %t7, !dbg !8
  store i64 %t3, ptr %t8, align 8, !dbg !8
  %t9 = add i64 %t5, 1, !dbg !8
  %t10 = getelementptr inbounds i8, ptr %t2, i64 8, !dbg !8
  store i64 %t9, ptr %t10, align 8, !dbg !8
  %t11 = getelementptr inbounds i8, ptr %t2, i64 24, !dbg !11
  %t12 = load i64, ptr %t11, align 8, !dbg !11
  %t13 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t12, i64 1), !dbg !11
  %t14 = extractvalue { i64, i1 } %t13, 1, !dbg !11
  br i1 %t14, label %L15, label %L16, !dbg !11
L15:
  store ptr @.str.0, ptr %t17, align 8, !dbg !11
  %t18 = getelementptr inbounds i8, ptr %t17, i64 8, !dbg !11
  store i64 27, ptr %t18, align 8, !dbg !11
  %t19 = getelementptr inbounds i8, ptr %t17, i64 16, !dbg !11
  store i64 27, ptr %t19, align 8, !dbg !11
  store ptr @.str.1, ptr %t20, align 8, !dbg !11
  %t21 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !11
  store i64 1, ptr %t21, align 8, !dbg !11
  %t22 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !11
  store i64 1, ptr %t22, align 8, !dbg !11
  call void @ignite_rt_fail_arith(ptr %t20, ptr %t17, i64 16, i64 16, i64 %t12, i64 1), !dbg !11
  unreachable, !dbg !11
L16:
  %t23 = extractvalue { i64, i1 } %t13, 0, !dbg !11
  %t24 = getelementptr inbounds i8, ptr %t2, i64 24, !dbg !10
  store i64 %t23, ptr %t24, align 8, !dbg !10
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t2, i64 32, i1 false), !dbg !5
  ret void, !dbg !5
}

define i64 @"tests.slice16.size"(ptr noalias readonly %p0) !dbg !14 {
entry:
  %t9 = alloca [24 x i8], align 8
  %t12 = alloca [24 x i8], align 8
  %t1 = getelementptr inbounds i8, ptr %p0, i64 24, !dbg !15
  %t2 = load i64, ptr %t1, align 8, !dbg !15
  %t3 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !16
  %t4 = load i64, ptr %t3, align 8, !dbg !16
  %t5 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t2, i64 %t4), !dbg !15
  %t6 = extractvalue { i64, i1 } %t5, 1, !dbg !15
  br i1 %t6, label %L7, label %L8, !dbg !15
L7:
  store ptr @.str.0, ptr %t9, align 8, !dbg !15
  %t10 = getelementptr inbounds i8, ptr %t9, i64 8, !dbg !15
  store i64 27, ptr %t10, align 8, !dbg !15
  %t11 = getelementptr inbounds i8, ptr %t9, i64 16, !dbg !15
  store i64 27, ptr %t11, align 8, !dbg !15
  store ptr @.str.1, ptr %t12, align 8, !dbg !15
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !15
  store i64 1, ptr %t13, align 8, !dbg !15
  %t14 = getelementptr inbounds i8, ptr %t12, i64 16, !dbg !15
  store i64 1, ptr %t14, align 8, !dbg !15
  call void @ignite_rt_fail_arith(ptr %t12, ptr %t9, i64 20, i64 32, i64 %t2, i64 %t4), !dbg !15
  unreachable, !dbg !15
L8:
  %t15 = extractvalue { i64, i1 } %t5, 0, !dbg !15
  ret i64 %t15, !dbg !15
}

define void @"tests.slice16.run"(ptr %sret, i64 %p0) !dbg !17 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t2 = alloca [32 x i8], align 8
  %t3 = alloca [24 x i8], align 8
  %t7 = alloca [32 x i8], align 8
  %t8 = alloca [8 x i8], align 8
  %t9 = alloca [32 x i8], align 8
  %t11 = alloca [8 x i8], align 8
  %t20 = alloca [32 x i8], align 8
  %t26 = alloca [24 x i8], align 8
  %t29 = alloca [24 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !18
  store ptr @ignite.empty, ptr %t3, align 8, !dbg !21
  %t4 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !21
  store i64 0, ptr %t4, align 8, !dbg !21
  %t5 = getelementptr inbounds i8, ptr %t3, i64 16, !dbg !21
  store i64 0, ptr %t5, align 8, !dbg !21
  call void @llvm.memcpy.p0.p0.i64(ptr %t2, ptr %t3, i64 24, i1 false), !dbg !20
  %t6 = getelementptr inbounds i8, ptr %t2, i64 24, !dbg !20
  store i64 0, ptr %t6, align 8, !dbg !20
  call void @llvm.memcpy.p0.p0.i64(ptr %t7, ptr %t2, i64 32, i1 false), !dbg !19
  store i64 0, ptr %t8, align 8, !dbg !23
  %t10 = call i64 @ignite.arena_mark(), !dbg !25
  store i64 0, ptr %t11, align 8, !dbg !25
  br label %L12
L12:
  %t16 = load i64, ptr %t8, align 8, !dbg !26
  %t17 = load i64, ptr %t1, align 8, !dbg !27
  %t18 = icmp slt i64 %t16, %t17, !dbg !26
  br i1 %t18, label %L13, label %L15, !dbg !25
L13:
  %t19 = load i64, ptr %t8, align 8, !dbg !31
  call void @"tests.slice16.step"(ptr %t20, ptr %t7, i64 %t19), !dbg !29
  call void @llvm.memcpy.p0.p0.i64(ptr %t7, ptr %t20, i64 32, i1 false), !dbg !28
  %t21 = load i64, ptr %t8, align 8, !dbg !33
  %t22 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t21, i64 1), !dbg !33
  %t23 = extractvalue { i64, i1 } %t22, 1, !dbg !33
  br i1 %t23, label %L24, label %L25, !dbg !33
L24:
  store ptr @.str.0, ptr %t26, align 8, !dbg !33
  %t27 = getelementptr inbounds i8, ptr %t26, i64 8, !dbg !33
  store i64 27, ptr %t27, align 8, !dbg !33
  %t28 = getelementptr inbounds i8, ptr %t26, i64 16, !dbg !33
  store i64 27, ptr %t28, align 8, !dbg !33
  store ptr @.str.1, ptr %t29, align 8, !dbg !33
  %t30 = getelementptr inbounds i8, ptr %t29, i64 8, !dbg !33
  store i64 1, ptr %t30, align 8, !dbg !33
  %t31 = getelementptr inbounds i8, ptr %t29, i64 16, !dbg !33
  store i64 1, ptr %t31, align 8, !dbg !33
  call void @ignite_rt_fail_arith(ptr %t29, ptr %t26, i64 27, i64 9, i64 %t21, i64 1), !dbg !33
  unreachable, !dbg !33
L25:
  %t32 = extractvalue { i64, i1 } %t22, 0, !dbg !33
  store i64 %t32, ptr %t8, align 8, !dbg !32
  br label %L14
L14:
  %t33 = load i64, ptr %t11, align 8, !dbg !25
  %t34 = call i1 @ignite.arena_due(i64 %t10, i64 %t33), !dbg !25
  br i1 %t34, label %L35, label %L12, !dbg !25
L35:
  call void @ignite_rt_arena_scratch(i8 1), !dbg !25
  call void @"copy.tests.slice16.State"(ptr %t9, ptr %t7), !dbg !25
  call void @ignite_rt_arena_scratch(i8 0), !dbg !25
  call void @ignite_rt_arena_release(i64 %t10), !dbg !25
  call void @"copy.tests.slice16.State"(ptr %t7, ptr %t9), !dbg !25
  call void @ignite_rt_arena_scratch_clear(), !dbg !25
  %t36 = call i64 @ignite.arena_mark(), !dbg !25
  %t37 = sub i64 %t36, %t10, !dbg !25
  store i64 %t37, ptr %t11, align 8, !dbg !25
  br label %L12, !dbg !25
L15:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t7, i64 32, i1 false), !dbg !18
  ret void, !dbg !18
}

define i64 @"tests.slice16.run_read_after"(i64 %p0) !dbg !36 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [32 x i8], align 8
  %t4 = alloca [32 x i8], align 8
  %t5 = alloca [32 x i8], align 8
  %t7 = alloca [32 x i8], align 8
  %t8 = alloca [32 x i8], align 8
  %t17 = alloca [24 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !37
  %t2 = load i64, ptr %t1, align 8, !dbg !40
  call void @"tests.slice16.run"(ptr %t3, i64 %t2), !dbg !39
  call void @llvm.memcpy.p0.p0.i64(ptr %t4, ptr %t3, i64 32, i1 false), !dbg !38
  call void @"copy.tests.slice16.State"(ptr %t5, ptr %t4), !dbg !42
  %t6 = load i64, ptr %t1, align 8, !dbg !44
  call void @"tests.slice16.step"(ptr %t7, ptr %t5, i64 %t6), !dbg !42
  call void @llvm.memcpy.p0.p0.i64(ptr %t8, ptr %t7, i64 32, i1 false), !dbg !41
  %t9 = getelementptr inbounds i8, ptr %t8, i64 24, !dbg !45
  %t10 = load i64, ptr %t9, align 8, !dbg !45
  %t11 = getelementptr inbounds i8, ptr %t4, i64 24, !dbg !46
  %t12 = load i64, ptr %t11, align 8, !dbg !46
  %t13 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t10, i64 %t12), !dbg !45
  %t14 = extractvalue { i64, i1 } %t13, 1, !dbg !45
  br i1 %t14, label %L15, label %L16, !dbg !45
L15:
  store ptr @.str.0, ptr %t17, align 8, !dbg !45
  %t18 = getelementptr inbounds i8, ptr %t17, i64 8, !dbg !45
  store i64 27, ptr %t18, align 8, !dbg !45
  %t19 = getelementptr inbounds i8, ptr %t17, i64 16, !dbg !45
  store i64 27, ptr %t19, align 8, !dbg !45
  store ptr @.str.1, ptr %t20, align 8, !dbg !45
  %t21 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !45
  store i64 1, ptr %t21, align 8, !dbg !45
  %t22 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !45
  store i64 1, ptr %t22, align 8, !dbg !45
  call void @ignite_rt_fail_arith(ptr %t20, ptr %t17, i64 35, i64 3, i64 %t10, i64 %t12), !dbg !45
  unreachable, !dbg !45
L16:
  %t23 = extractvalue { i64, i1 } %t13, 0, !dbg !45
  ret i64 %t23, !dbg !37
}

define i64 @"tests.slice16.read_only"(i64 %p0) !dbg !47 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [32 x i8], align 8
  %t4 = alloca [32 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !48
  %t2 = load i64, ptr %t1, align 8, !dbg !51
  call void @"tests.slice16.run"(ptr %t3, i64 %t2), !dbg !50
  call void @llvm.memcpy.p0.p0.i64(ptr %t4, ptr %t3, i64 32, i1 false), !dbg !49
  %t5 = call i64 @"tests.slice16.size"(ptr %t4), !dbg !52
  ret i64 %t5, !dbg !48
}

define internal void @"fnval.tests.slice16.step"(ptr %env, ptr %a0, ptr %a1, i64 %a2) {
entry:
  call void @"tests.slice16.step"(ptr %a0, ptr %a1, i64 %a2)
  ret void
}

define void @"tests.slice16.run_actor"(ptr %sret, i64 %p0) !dbg !54 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t2 = alloca [48 x i8], align 8
  %t3 = alloca [32 x i8], align 8
  %t4 = alloca [24 x i8], align 8
  %t8 = alloca [16 x i8], align 8
  %t11 = alloca [48 x i8], align 8
  %t12 = alloca [8 x i8], align 8
  %t13 = alloca [48 x i8], align 8
  %t15 = alloca [8 x i8], align 8
  %t28 = alloca [32 x i8], align 8
  %t34 = alloca [24 x i8], align 8
  %t37 = alloca [24 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !55
  store ptr @ignite.empty, ptr %t4, align 8, !dbg !59
  %t5 = getelementptr inbounds i8, ptr %t4, i64 8, !dbg !59
  store i64 0, ptr %t5, align 8, !dbg !59
  %t6 = getelementptr inbounds i8, ptr %t4, i64 16, !dbg !59
  store i64 0, ptr %t6, align 8, !dbg !59
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t4, i64 24, i1 false), !dbg !58
  %t7 = getelementptr inbounds i8, ptr %t3, i64 24, !dbg !58
  store i64 0, ptr %t7, align 8, !dbg !58
  call void @llvm.memcpy.p0.p0.i64(ptr %t2, ptr %t3, i64 32, i1 false), !dbg !57
  store ptr @"fnval.tests.slice16.step", ptr %t8, align 8, !dbg !61
  %t9 = getelementptr inbounds i8, ptr %t8, i64 8, !dbg !61
  store ptr null, ptr %t9, align 8, !dbg !61
  %t10 = getelementptr inbounds i8, ptr %t2, i64 32, !dbg !57
  call void @llvm.memcpy.p0.p0.i64(ptr %t10, ptr %t8, i64 16, i1 false), !dbg !57
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t2, i64 48, i1 false), !dbg !56
  store i64 0, ptr %t12, align 8, !dbg !62
  %t14 = call i64 @ignite.arena_mark(), !dbg !64
  store i64 0, ptr %t15, align 8, !dbg !64
  br label %L16
L16:
  %t20 = load i64, ptr %t12, align 8, !dbg !65
  %t21 = load i64, ptr %t1, align 8, !dbg !66
  %t22 = icmp slt i64 %t20, %t21, !dbg !65
  br i1 %t22, label %L17, label %L19, !dbg !64
L17:
  %t23 = getelementptr inbounds i8, ptr %t11, i64 32, !dbg !68
  %t24 = load ptr, ptr %t23, align 8, !dbg !68
  %t25 = getelementptr inbounds i8, ptr %t23, i64 8, !dbg !68
  %t26 = load ptr, ptr %t25, align 8, !dbg !68
  %t27 = load i64, ptr %t12, align 8, !dbg !70
  call void %t24(ptr %t26, ptr %t28, ptr %t11, i64 %t27), !dbg !68
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t28, i64 32, i1 false), !dbg !67
  %t29 = load i64, ptr %t12, align 8, !dbg !72
  %t30 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t29, i64 1), !dbg !72
  %t31 = extractvalue { i64, i1 } %t30, 1, !dbg !72
  br i1 %t31, label %L32, label %L33, !dbg !72
L32:
  store ptr @.str.0, ptr %t34, align 8, !dbg !72
  %t35 = getelementptr inbounds i8, ptr %t34, i64 8, !dbg !72
  store i64 27, ptr %t35, align 8, !dbg !72
  %t36 = getelementptr inbounds i8, ptr %t34, i64 16, !dbg !72
  store i64 27, ptr %t36, align 8, !dbg !72
  store ptr @.str.1, ptr %t37, align 8, !dbg !72
  %t38 = getelementptr inbounds i8, ptr %t37, i64 8, !dbg !72
  store i64 1, ptr %t38, align 8, !dbg !72
  %t39 = getelementptr inbounds i8, ptr %t37, i64 16, !dbg !72
  store i64 1, ptr %t39, align 8, !dbg !72
  call void @ignite_rt_fail_arith(ptr %t37, ptr %t34, i64 48, i64 9, i64 %t29, i64 1), !dbg !72
  unreachable, !dbg !72
L33:
  %t40 = extractvalue { i64, i1 } %t30, 0, !dbg !72
  store i64 %t40, ptr %t12, align 8, !dbg !71
  br label %L18
L18:
  %t41 = load i64, ptr %t15, align 8, !dbg !64
  %t42 = call i1 @ignite.arena_due(i64 %t14, i64 %t41), !dbg !64
  br i1 %t42, label %L43, label %L16, !dbg !64
L43:
  call void @ignite_rt_arena_scratch(i8 1), !dbg !64
  call void @"copy.tests.slice16.Actor"(ptr %t13, ptr %t11), !dbg !64
  call void @ignite_rt_arena_scratch(i8 0), !dbg !64
  call void @ignite_rt_arena_release(i64 %t14), !dbg !64
  call void @"copy.tests.slice16.Actor"(ptr %t11, ptr %t13), !dbg !64
  call void @ignite_rt_arena_scratch_clear(), !dbg !64
  %t44 = call i64 @ignite.arena_mark(), !dbg !64
  %t45 = sub i64 %t44, %t14, !dbg !64
  store i64 %t45, ptr %t15, align 8, !dbg !64
  br label %L16, !dbg !64
L19:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t11, i64 48, i1 false), !dbg !55
  ret void, !dbg !55
}

define internal i64 @"closure.tests.slice16.1"(ptr %env, i64 %p0) !dbg !77 {
entry:
  %t3 = alloca [8 x i8], align 8
  %t4 = alloca [8 x i8], align 8
  %t11 = alloca [24 x i8], align 8
  %t14 = alloca [24 x i8], align 8
  %t1 = getelementptr inbounds i8, ptr %env, i64 8, !dbg !78
  %t2 = load i64, ptr %t1, align 8, !dbg !78
  store i64 %t2, ptr %t3, align 8, !dbg !78
  store i64 %p0, ptr %t4, align 8, !dbg !78
  %t5 = load i64, ptr %t4, align 8, !dbg !79
  %t6 = load i64, ptr %t3, align 8, !dbg !80
  %t7 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t5, i64 %t6), !dbg !79
  %t8 = extractvalue { i64, i1 } %t7, 1, !dbg !79
  br i1 %t8, label %L9, label %L10, !dbg !79
L9:
  store ptr @.str.0, ptr %t11, align 8, !dbg !79
  %t12 = getelementptr inbounds i8, ptr %t11, i64 8, !dbg !79
  store i64 27, ptr %t12, align 8, !dbg !79
  %t13 = getelementptr inbounds i8, ptr %t11, i64 16, !dbg !79
  store i64 27, ptr %t13, align 8, !dbg !79
  store ptr @.str.1, ptr %t14, align 8, !dbg !79
  %t15 = getelementptr inbounds i8, ptr %t14, i64 8, !dbg !79
  store i64 1, ptr %t15, align 8, !dbg !79
  %t16 = getelementptr inbounds i8, ptr %t14, i64 16, !dbg !79
  store i64 1, ptr %t16, align 8, !dbg !79
  call void @ignite_rt_fail_arith(ptr %t14, ptr %t11, i64 53, i64 42, i64 %t5, i64 %t6), !dbg !79
  unreachable, !dbg !79
L10:
  %t17 = extractvalue { i64, i1 } %t7, 0, !dbg !79
  ret i64 %t17, !dbg !78
}

define internal ptr @"closure.tests.slice16.1.env"(i32 %op, ptr %src) {
entry:
  switch i32 %op, label %L1 [ i32 1, label %L2 i32 2, label %L3 ]
L1:
  %t4 = call ptr @ignite.alloc(i64 16, i64 8)
  call void @llvm.memcpy.p0.p0.i64(ptr %t4, ptr %src, i64 16, i1 false)
  ret ptr %t4
L2:
  %t5 = call ptr @malloc(i64 16)
  call void @llvm.memcpy.p0.p0.i64(ptr %t5, ptr %src, i64 16, i1 false)
  ret ptr %t5
L3:
  call void @free(ptr %src)
  ret ptr null
}

define void @"tests.slice16.adder"(ptr %sret, i64 %p0) !dbg !75 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t2 = alloca [16 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !76
  store ptr @"closure.tests.slice16.1", ptr %t2, align 8, !dbg !76
  %t3 = call ptr @ignite.alloc(i64 16, i64 8), !dbg !76
  store ptr @"closure.tests.slice16.1.env", ptr %t3, align 8, !dbg !76
  %t4 = load i64, ptr %t1, align 8, !dbg !76
  %t5 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !76
  store i64 %t4, ptr %t5, align 8, !dbg !76
  %t6 = getelementptr inbounds i8, ptr %t2, i64 8, !dbg !76
  store ptr %t3, ptr %t6, align 8, !dbg !76
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t2, i64 16, i1 false), !dbg !76
  ret void, !dbg !76
}

define internal i64 @"closure.tests.slice16.2"(ptr %env, i64 %p0) !dbg !97 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t7 = alloca [24 x i8], align 8
  %t10 = alloca [24 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !98
  %t2 = load i64, ptr %t1, align 8, !dbg !99
  %t3 = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %t2, i64 3), !dbg !99
  %t4 = extractvalue { i64, i1 } %t3, 1, !dbg !99
  br i1 %t4, label %L5, label %L6, !dbg !99
L5:
  store ptr @.str.0, ptr %t7, align 8, !dbg !99
  %t8 = getelementptr inbounds i8, ptr %t7, i64 8, !dbg !99
  store i64 27, ptr %t8, align 8, !dbg !99
  %t9 = getelementptr inbounds i8, ptr %t7, i64 16, !dbg !99
  store i64 27, ptr %t9, align 8, !dbg !99
  store ptr @.str.2, ptr %t10, align 8, !dbg !99
  %t11 = getelementptr inbounds i8, ptr %t10, i64 8, !dbg !99
  store i64 1, ptr %t11, align 8, !dbg !99
  %t12 = getelementptr inbounds i8, ptr %t10, i64 16, !dbg !99
  store i64 1, ptr %t12, align 8, !dbg !99
  call void @ignite_rt_fail_arith(ptr %t10, ptr %t7, i64 57, i64 39, i64 %t2, i64 3), !dbg !99
  unreachable, !dbg !99
L6:
  %t13 = extractvalue { i64, i1 } %t3, 0, !dbg !99
  ret i64 %t13, !dbg !98
}

define i64 @"tests.slice16.expression_calls"() !dbg !81 {
entry:
  %t2 = alloca [16 x i8], align 8
  %t3 = alloca [16 x i8], align 8
  %t5 = alloca [24 x i8], align 8
  %t8 = alloca [24 x i8], align 8
  %t9 = alloca [16 x i8], align 8
  %t19 = alloca [24 x i8], align 8
  %t33 = alloca [24 x i8], align 8
  %t36 = alloca [24 x i8], align 8
  %t1 = call ptr @ignite.alloc(i64 32, i64 8), !dbg !84
  call void @"tests.slice16.adder"(ptr %t2, i64 1), !dbg !85
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t2, i64 16, i1 false), !dbg !84
  call void @"tests.slice16.adder"(ptr %t3, i64 2), !dbg !87
  %t4 = getelementptr inbounds i8, ptr %t1, i64 16, !dbg !84
  call void @llvm.memcpy.p0.p0.i64(ptr %t4, ptr %t3, i64 16, i1 false), !dbg !84
  store ptr %t1, ptr %t5, align 8, !dbg !84
  %t6 = getelementptr inbounds i8, ptr %t5, i64 8, !dbg !84
  store i64 2, ptr %t6, align 8, !dbg !84
  %t7 = getelementptr inbounds i8, ptr %t5, i64 16, !dbg !84
  store i64 2, ptr %t7, align 8, !dbg !84
  call void @llvm.memcpy.p0.p0.i64(ptr %t8, ptr %t5, i64 24, i1 false), !dbg !83
  call void @"tests.slice16.adder"(ptr %t9, i64 1), !dbg !89
  %t10 = load ptr, ptr %t9, align 8, !dbg !89
  %t11 = getelementptr inbounds i8, ptr %t9, i64 8, !dbg !89
  %t12 = load ptr, ptr %t11, align 8, !dbg !89
  %t13 = call i64 %t10(ptr %t12, i64 2), !dbg !89
  %t14 = getelementptr inbounds i8, ptr %t8, i64 8, !dbg !92
  %t15 = load i64, ptr %t14, align 8, !dbg !92
  %t16 = icmp uge i64 1, %t15, !dbg !92
  br i1 %t16, label %L17, label %L18, !dbg !92
L17:
  store ptr @.str.0, ptr %t19, align 8, !dbg !92
  %t20 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !92
  store i64 27, ptr %t20, align 8, !dbg !92
  %t21 = getelementptr inbounds i8, ptr %t19, i64 16, !dbg !92
  store i64 27, ptr %t21, align 8, !dbg !92
  call void @ignite_rt_fail_index(ptr %t19, i64 57, i64 17, i64 1, i64 %t15), !dbg !92
  unreachable, !dbg !92
L18:
  %t22 = load ptr, ptr %t8, align 8, !dbg !92
  %t23 = mul i64 1, 16, !dbg !92
  %t24 = getelementptr inbounds i8, ptr %t22, i64 %t23, !dbg !92
  %t25 = load ptr, ptr %t24, align 8, !dbg !92
  %t26 = getelementptr inbounds i8, ptr %t24, i64 8, !dbg !92
  %t27 = load ptr, ptr %t26, align 8, !dbg !92
  %t28 = call i64 %t25(ptr %t27, i64 10), !dbg !92
  %t29 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t13, i64 %t28), !dbg !89
  %t30 = extractvalue { i64, i1 } %t29, 1, !dbg !89
  br i1 %t30, label %L31, label %L32, !dbg !89
L31:
  store ptr @.str.0, ptr %t33, align 8, !dbg !89
  %t34 = getelementptr inbounds i8, ptr %t33, i64 8, !dbg !89
  store i64 27, ptr %t34, align 8, !dbg !89
  %t35 = getelementptr inbounds i8, ptr %t33, i64 16, !dbg !89
  store i64 27, ptr %t35, align 8, !dbg !89
  store ptr @.str.1, ptr %t36, align 8, !dbg !89
  %t37 = getelementptr inbounds i8, ptr %t36, i64 8, !dbg !89
  store i64 1, ptr %t37, align 8, !dbg !89
  %t38 = getelementptr inbounds i8, ptr %t36, i64 16, !dbg !89
  store i64 1, ptr %t38, align 8, !dbg !89
  call void @ignite_rt_fail_arith(ptr %t36, ptr %t33, i64 57, i64 3, i64 %t13, i64 %t28), !dbg !89
  unreachable, !dbg !89
L32:
  %t39 = extractvalue { i64, i1 } %t29, 0, !dbg !89
  store ptr @"closure.tests.slice16.2", ptr %t9, align 8, !dbg !96
  %t40 = getelementptr inbounds i8, ptr %t9, i64 8, !dbg !96
  store ptr null, ptr %t40, align 8, !dbg !96
  %t41 = load ptr, ptr %t9, align 8, !dbg !95
  %t42 = getelementptr inbounds i8, ptr %t9, i64 8, !dbg !95
  %t43 = load ptr, ptr %t42, align 8, !dbg !95
  %t44 = call i64 %t41(ptr %t43, i64 2), !dbg !95
  %t45 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t39, i64 %t44), !dbg !89
  %t46 = extractvalue { i64, i1 } %t45, 1, !dbg !89
  br i1 %t46, label %L47, label %L48, !dbg !89
L47:
  store ptr @.str.0, ptr %t33, align 8, !dbg !89
  %t49 = getelementptr inbounds i8, ptr %t33, i64 8, !dbg !89
  store i64 27, ptr %t49, align 8, !dbg !89
  %t50 = getelementptr inbounds i8, ptr %t33, i64 16, !dbg !89
  store i64 27, ptr %t50, align 8, !dbg !89
  store ptr @.str.1, ptr %t19, align 8, !dbg !89
  %t51 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !89
  store i64 1, ptr %t51, align 8, !dbg !89
  %t52 = getelementptr inbounds i8, ptr %t19, i64 16, !dbg !89
  store i64 1, ptr %t52, align 8, !dbg !89
  call void @ignite_rt_fail_arith(ptr %t19, ptr %t33, i64 57, i64 3, i64 %t39, i64 %t44), !dbg !89
  unreachable, !dbg !89
L48:
  %t53 = extractvalue { i64, i1 } %t45, 0, !dbg !89
  ret i64 %t53, !dbg !82
}

define internal void @"copy.tests.slice16.Actor"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 48, i1 false)
  %t1 = getelementptr inbounds i8, ptr %dst, i64 0
  %t2 = getelementptr inbounds i8, ptr %src, i64 0
  call void @"copy.tests.slice16.State"(ptr %t1, ptr %t2)
  %t3 = getelementptr inbounds i8, ptr %dst, i64 32
  %t4 = getelementptr inbounds i8, ptr %src, i64 32
  call void @"copy.fn(tests.slice16.State,Int)->tests.slice16.State"(ptr %t3, ptr %t4)
  ret void
}

define internal void @"copy.fn(tests.slice16.State,Int)->tests.slice16.State"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 16, i1 false)
  %t1 = getelementptr inbounds i8, ptr %src, i64 8
  %t2 = load ptr, ptr %t1, align 8
  %t3 = icmp eq ptr %t2, null
  br i1 %t3, label %L5, label %L4
L4:
  %t6 = load ptr, ptr %t2, align 8
  %t7 = call ptr %t6(i32 0, ptr %t2)
  %t8 = getelementptr inbounds i8, ptr %dst, i64 8
  store ptr %t7, ptr %t8, align 8
  br label %L5
L5:
  ret void
}

define internal void @"copy.tests.slice16.State"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 32, i1 false)
  %t1 = getelementptr inbounds i8, ptr %dst, i64 0
  %t2 = getelementptr inbounds i8, ptr %src, i64 0
  call void @"copy.List<Int>"(ptr %t1, ptr %t2)
  ret void
}

define internal void @"copy.List<Int>"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 24, i1 false)
  %t1 = getelementptr inbounds i8, ptr %src, i64 8
  %t2 = load i64, ptr %t1, align 8
  %t3 = load ptr, ptr %src, align 8
  %t4 = icmp sle i64 %t2, 0
  br i1 %t4, label %L5, label %L6
L5:
  store ptr @ignite.empty, ptr %dst, align 8
  %t8 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 0, ptr %t8, align 8
  %t9 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 0, ptr %t9, align 8
  br label %L7
L6:
  %t10 = mul i64 %t2, 8
  %t11 = call ptr @ignite.alloc(i64 %t10, i64 8)
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t3, i64 %t10, i1 false)
  store ptr %t11, ptr %dst, align 8
  %t12 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 %t2, ptr %t12, align 8
  %t13 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 %t2, ptr %t13, align 8
  br label %L7
L7:
  ret void
}

!llvm.dbg.cu = !{!1}
!llvm.module.flags = !{!102, !103}
!0 = !DIFile(filename: "tests.slice16.ll", directory: ".")
!1 = distinct !DICompileUnit(language: DW_LANG_C, file: !0, producer: "ignite", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, nameTableKind: None)
!2 = !DISubroutineType(types: !{})
!3 = !DIFile(filename: "slice16.ignite", directory: "./tests/golden")
!4 = distinct !DISubprogram(name: "tests.slice16.step", linkageName: "tests.slice16.step", scope: !3, file: !3, line: 13, type: !2, scopeLine: 13, spFlags: DISPFlagDefinition, unit: !1)
!5 = !DILocation(line: 13, column: 40, scope: !4)
!6 = !DILocation(line: 14, column: 3, scope: !4)
!7 = !DILocation(line: 14, column: 14, scope: !4)
!8 = !DILocation(line: 15, column: 3, scope: !4)
!9 = !DILocation(line: 15, column: 19, scope: !4)
!10 = !DILocation(line: 16, column: 3, scope: !4)
!11 = !DILocation(line: 16, column: 16, scope: !4)
!12 = !DILocation(line: 16, column: 29, scope: !4)
!13 = !DILocation(line: 17, column: 3, scope: !4)
!14 = distinct !DISubprogram(name: "tests.slice16.size", linkageName: "tests.slice16.size", scope: !3, file: !3, line: 20, type: !2, scopeLine: 20, spFlags: DISPFlagDefinition, unit: !1)
!15 = !DILocation(line: 20, column: 32, scope: !14)
!16 = !DILocation(line: 20, column: 46, scope: !14)
!17 = distinct !DISubprogram(name: "tests.slice16.run", linkageName: "tests.slice16.run", scope: !3, file: !3, line: 22, type: !2, scopeLine: 22, spFlags: DISPFlagDefinition, unit: !1)
!18 = !DILocation(line: 22, column: 25, scope: !17)
!19 = !DILocation(line: 23, column: 3, scope: !17)
!20 = !DILocation(line: 23, column: 11, scope: !17)
!21 = !DILocation(line: 23, column: 27, scope: !17)
!22 = !DILocation(line: 23, column: 39, scope: !17)
!23 = !DILocation(line: 24, column: 3, scope: !17)
!24 = !DILocation(line: 24, column: 11, scope: !17)
!25 = !DILocation(line: 25, column: 3, scope: !17)
!26 = !DILocation(line: 25, column: 9, scope: !17)
!27 = !DILocation(line: 25, column: 13, scope: !17)
!28 = !DILocation(line: 26, column: 5, scope: !17)
!29 = !DILocation(line: 26, column: 9, scope: !17)
!30 = !DILocation(line: 26, column: 14, scope: !17)
!31 = !DILocation(line: 26, column: 17, scope: !17)
!32 = !DILocation(line: 27, column: 5, scope: !17)
!33 = !DILocation(line: 27, column: 9, scope: !17)
!34 = !DILocation(line: 27, column: 13, scope: !17)
!35 = !DILocation(line: 29, column: 3, scope: !17)
!36 = distinct !DISubprogram(name: "tests.slice16.run_read_after", linkageName: "tests.slice16.run_read_after", scope: !3, file: !3, line: 32, type: !2, scopeLine: 32, spFlags: DISPFlagDefinition, unit: !1)
!37 = !DILocation(line: 32, column: 34, scope: !36)
!38 = !DILocation(line: 33, column: 3, scope: !36)
!39 = !DILocation(line: 33, column: 11, scope: !36)
!40 = !DILocation(line: 33, column: 15, scope: !36)
!41 = !DILocation(line: 34, column: 3, scope: !36)
!42 = !DILocation(line: 34, column: 11, scope: !36)
!43 = !DILocation(line: 34, column: 16, scope: !36)
!44 = !DILocation(line: 34, column: 19, scope: !36)
!45 = !DILocation(line: 35, column: 3, scope: !36)
!46 = !DILocation(line: 35, column: 13, scope: !36)
!47 = distinct !DISubprogram(name: "tests.slice16.read_only", linkageName: "tests.slice16.read_only", scope: !3, file: !3, line: 38, type: !2, scopeLine: 38, spFlags: DISPFlagDefinition, unit: !1)
!48 = !DILocation(line: 38, column: 29, scope: !47)
!49 = !DILocation(line: 39, column: 3, scope: !47)
!50 = !DILocation(line: 39, column: 11, scope: !47)
!51 = !DILocation(line: 39, column: 15, scope: !47)
!52 = !DILocation(line: 40, column: 3, scope: !47)
!53 = !DILocation(line: 40, column: 8, scope: !47)
!54 = distinct !DISubprogram(name: "tests.slice16.run_actor", linkageName: "tests.slice16.run_actor", scope: !3, file: !3, line: 43, type: !2, scopeLine: 43, spFlags: DISPFlagDefinition, unit: !1)
!55 = !DILocation(line: 43, column: 31, scope: !54)
!56 = !DILocation(line: 44, column: 3, scope: !54)
!57 = !DILocation(line: 44, column: 11, scope: !54)
!58 = !DILocation(line: 44, column: 27, scope: !54)
!59 = !DILocation(line: 44, column: 43, scope: !54)
!60 = !DILocation(line: 44, column: 55, scope: !54)
!61 = !DILocation(line: 44, column: 69, scope: !54)
!62 = !DILocation(line: 45, column: 3, scope: !54)
!63 = !DILocation(line: 45, column: 11, scope: !54)
!64 = !DILocation(line: 46, column: 3, scope: !54)
!65 = !DILocation(line: 46, column: 9, scope: !54)
!66 = !DILocation(line: 46, column: 13, scope: !54)
!67 = !DILocation(line: 47, column: 5, scope: !54)
!68 = !DILocation(line: 47, column: 15, scope: !54)
!69 = !DILocation(line: 47, column: 24, scope: !54)
!70 = !DILocation(line: 47, column: 33, scope: !54)
!71 = !DILocation(line: 48, column: 5, scope: !54)
!72 = !DILocation(line: 48, column: 9, scope: !54)
!73 = !DILocation(line: 48, column: 13, scope: !54)
!74 = !DILocation(line: 50, column: 3, scope: !54)
!75 = distinct !DISubprogram(name: "tests.slice16.adder", linkageName: "tests.slice16.adder", scope: !3, file: !3, line: 53, type: !2, scopeLine: 53, spFlags: DISPFlagDefinition, unit: !1)
!76 = !DILocation(line: 53, column: 38, scope: !75)
!77 = distinct !DISubprogram(name: "closure.tests.slice16.1", linkageName: "closure.tests.slice16.1", scope: !3, file: !3, line: 53, type: !2, scopeLine: 53, spFlags: DISPFlagDefinition, unit: !1)
!78 = !DILocation(line: 53, column: 38, scope: !77)
!79 = !DILocation(line: 53, column: 42, scope: !77)
!80 = !DILocation(line: 53, column: 46, scope: !77)
!81 = distinct !DISubprogram(name: "tests.slice16.expression_calls", linkageName: "tests.slice16.expression_calls", scope: !3, file: !3, line: 55, type: !2, scopeLine: 55, spFlags: DISPFlagDefinition, unit: !1)
!82 = !DILocation(line: 55, column: 30, scope: !81)
!83 = !DILocation(line: 56, column: 3, scope: !81)
!84 = !DILocation(line: 56, column: 12, scope: !81)
!85 = !DILocation(line: 56, column: 13, scope: !81)
!86 = !DILocation(line: 56, column: 19, scope: !81)
!87 = !DILocation(line: 56, column: 23, scope: !81)
!88 = !DILocation(line: 56, column: 29, scope: !81)
!89 = !DILocation(line: 57, column: 3, scope: !81)
!90 = !DILocation(line: 57, column: 9, scope: !81)
!91 = !DILocation(line: 57, column: 12, scope: !81)
!92 = !DILocation(line: 57, column: 17, scope: !81)
!93 = !DILocation(line: 57, column: 20, scope: !81)
!94 = !DILocation(line: 57, column: 23, scope: !81)
!95 = !DILocation(line: 57, column: 29, scope: !81)
!96 = !DILocation(line: 57, column: 30, scope: !81)
!97 = distinct !DISubprogram(name: "closure.tests.slice16.2", linkageName: "closure.tests.slice16.2", scope: !3, file: !3, line: 57, type: !2, scopeLine: 57, spFlags: DISPFlagDefinition, unit: !1)
!98 = !DILocation(line: 57, column: 30, scope: !97)
!99 = !DILocation(line: 57, column: 39, scope: !97)
!100 = !DILocation(line: 57, column: 43, scope: !97)
!101 = !DILocation(line: 57, column: 46, scope: !81)
!102 = !{i32 7, !"Dwarf Version", i32 5}
!103 = !{i32 2, !"Debug Info Version", i32 3}
