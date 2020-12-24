using System;
using System.Collections.Generic;
using System.Diagnostics;
using Terminal.Gui;

namespace PACTUniversal
{
    public class Program
    {
        private static Toplevel toplevel;

        private static MenuBar menuBar;
        private static Window statusWindow;
        private static Window configWindow;
        private static Window optionsWindow;


        private static Label Label_Status_TotalCpuUsageText;
        private static Label Label_Status_TotalCpuUsage;

        private static Label Label_Status_ProcessCountText;
        private static Label Label_Status_ProcessCount;

        private static Label Label_Status_HighPerformanceCountText;
        private static Label Label_Status_HighPerformanceCount;

        private static Label Label_Status_CustomPerformanceCountText;
        private static Label Label_Status_CustomPerformanceCount;

        private static Label Label_Status_InaccessibleCountText;
        private static Label Label_Status_InaccessibleCount;

        private static Button Button_Status_Toggle;
        private static Label Label_Status_Toggle;

        private static Label Label_Status_MadeBy;

        public static void Main()
        {
            Init();
        }

        private static void Init()
        {
            Application.Init();
            toplevel = Application.Top;

            InitTopBar();
            toplevel.Add(menuBar);

            InitStatusWindow();
            toplevel.Add(statusWindow);

            ShowWindow(statusWindow);

            Application.Run();
        }

        private static void InitTopBar()
        {
            List<MenuBarItem> menuBarItems = new List<MenuBarItem>();

            MenuBarItem status = new MenuBarItem();
            status.Title = "Status";
            status.Action = () => ShowWindow(statusWindow);
            menuBarItems.Add(status);

            MenuBarItem configure = new MenuBarItem();
            configure.Title = "Configure";
            configure.Action = () => ShowWindow(configWindow);
            menuBarItems.Add(configure);

            MenuBarItem options = new MenuBarItem();
            options.Title = "Help & Options";
            options.Action = () => ShowWindow(optionsWindow);
            menuBarItems.Add(options);

            menuBar = new MenuBar(menuBarItems.ToArray());
        }

        private static void InitStatusWindow()
        {
            statusWindow = new Window();
            statusWindow.Title = "Status";
            statusWindow.X = 0;
            statusWindow.Y = 1; // One down for the menu-bar.
            statusWindow.Width = Dim.Fill();
            statusWindow.Height = Dim.Fill();

            Label_Status_TotalCpuUsageText = new Label();
            Label_Status_TotalCpuUsageText.Text = "Total CPU Usage:";
            Label_Status_TotalCpuUsageText.X = 0;
            Label_Status_TotalCpuUsageText.Y = 2;
            statusWindow.Add(Label_Status_TotalCpuUsageText);

            Label_Status_TotalCpuUsage = new Label();
            Label_Status_TotalCpuUsage.Text = "100%";
            Label_Status_TotalCpuUsage.X = 20;
            Label_Status_TotalCpuUsage.Y = 2;
            statusWindow.Add(Label_Status_TotalCpuUsage);



            Label_Status_ProcessCountText = new Label();
            Label_Status_ProcessCountText.Text = "Total Processes:";
            Label_Status_ProcessCountText.X = 0;
            Label_Status_ProcessCountText.Y = 3;
            statusWindow.Add(Label_Status_ProcessCountText);

            Label_Status_ProcessCount = new Label();
            Label_Status_ProcessCount.Text = "9999";
            Label_Status_ProcessCount.X = 20;
            Label_Status_ProcessCount.Y = 3;
            statusWindow.Add(Label_Status_ProcessCount);



            Label_Status_HighPerformanceCountText = new Label();
            Label_Status_HighPerformanceCountText.Text = "High Perf. Processes:";
            Label_Status_HighPerformanceCountText.X = 0;
            Label_Status_HighPerformanceCountText.Y = 4;
            statusWindow.Add(Label_Status_HighPerformanceCountText);

            Label_Status_HighPerformanceCount = new Label();
            Label_Status_HighPerformanceCount.Text = "9999";
            Label_Status_HighPerformanceCount.X = 20;
            Label_Status_HighPerformanceCount.Y = 4;
            statusWindow.Add(Label_Status_HighPerformanceCount);



            Label_Status_CustomPerformanceCountText = new Label();
            Label_Status_CustomPerformanceCountText.Text = "Custom Perf. Processes";
            Label_Status_CustomPerformanceCountText.X = 0;
            Label_Status_CustomPerformanceCountText.Y = 5;
            statusWindow.Add(Label_Status_CustomPerformanceCountText);

            Label_Status_CustomPerformanceCount = new Label();
            Label_Status_CustomPerformanceCount.Text = "9999";
            Label_Status_CustomPerformanceCount.X = 20;
            Label_Status_CustomPerformanceCount.Y = 5;
            statusWindow.Add(Label_Status_CustomPerformanceCount);



            Label_Status_InaccessibleCountText = new Label();
            Label_Status_InaccessibleCountText.Text = "Inaccessible Processes:";
            Label_Status_InaccessibleCountText.X = 00;
            Label_Status_InaccessibleCountText.Y = 6;
            statusWindow.Add(Label_Status_InaccessibleCountText);

            Label_Status_InaccessibleCount = new Label();
            Label_Status_InaccessibleCount.Text = "9999";
            Label_Status_InaccessibleCount.X = 20;
            Label_Status_InaccessibleCount.Y = 6;
            statusWindow.Add(Label_Status_InaccessibleCount);



            Button_Status_Toggle = new Button();
            Button_Status_Toggle.Text = "Toggle";
            Button_Status_Toggle.X = 0;
            Button_Status_Toggle.Y = 7;
            statusWindow.Add(Button_Status_Toggle);

            Label_Status_Toggle = new Label();
            Label_Status_Toggle.Text = "PACT is ACTIVE";
            Label_Status_Toggle.X = 20;
            Label_Status_Toggle.Y = 7;
            statusWindow.Add(Label_Status_Toggle);



            Label_Status_MadeBy = new Label();
            Label_Status_MadeBy.Text = "Made by Berk (SAS41) Alyamach";
            Label_Status_MadeBy.X = 0;
            Label_Status_MadeBy.Y = 8;
            statusWindow.Add(Label_Status_MadeBy);
        }

        private static void InitConfigWindow()
        {
            configWindow = new Window("Configure");
            configWindow.X = 0;
            configWindow.Y = 1; // One down for the menu-bar.
            configWindow.Width = Dim.Fill();
            configWindow.Height = Dim.Fill();
        }

        private static void InitOptionsWindow()
        {
            optionsWindow = new Window("Help & Options");
            optionsWindow.X = 0;
            optionsWindow.Y = 1; // One down for the menu-bar.
            optionsWindow.Width = Dim.Fill();
            optionsWindow.Height = Dim.Fill();
        }

        private static void ShowWindow(Window window)
        {
            window.SetFocus();
            window.ChildNeedsDisplay();
            toplevel.BringSubviewToFront(window);
        }
    }
}