using System;
using System.Collections.Generic;
using System.Text;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shapes;
using System.Linq;
using System.Diagnostics;
using PACTCore;

namespace PACTWPF
{
    /// <summary>
    /// Interaction logic for ProcessConfigEditWindow.xaml
    /// </summary>
    public partial class ProcessConfigEditWindow : Window
    {
        public string TargetProcessOrGroup { get; set; }

        private List<CheckBox> CheckBoxes;

        public ProcessConfigEditWindow(ProcessConfig initial)
        {
            CheckBoxes = new List<CheckBox>();
            InitializeComponent();

            foreach (var item in initial.CoreList)
            {
                CheckBoxes[item].IsChecked = true;
            }

            ComboBox_PrioritySelect.SelectedItem = initial.Priority;
        }

        private void Grid_ProcessConfigEditWindow_CPUSelect_Initialized(object sender, EventArgs e)
        {
            int threadCount = Environment.ProcessorCount;

            int columns = 2;
            int rows = 1;
            int gridSize = columns * rows;

            while (gridSize < threadCount)
            {
                if (rows <= columns)
                {
                    rows++;
                }
                else
                {
                    columns += 2;
                }

                gridSize = columns * rows;
            }

            for (int i = 0; i < columns; i++)
            {
                Grid_ProcessConfigEditWindow_CPUSelect.ColumnDefinitions.Add(new ColumnDefinition());
            }

            for (int i = 0; i < rows; i++)
            {
                Grid_ProcessConfigEditWindow_CPUSelect.RowDefinitions.Add(new RowDefinition());
            }

            int assigned = 0;
            for (int i = 0; i < rows; i++)
            {
                for (int j = 0; j < columns; j++)
                {
                    if (assigned > threadCount)
                    {
                        break;
                    }

                    CheckBox cb = new CheckBox();
                    cb.Content = $"T: {assigned}";
                    cb.Checked += CPUSelectCheckBoxChanged;
                    cb.HorizontalAlignment = HorizontalAlignment.Center;
                    cb.VerticalAlignment = VerticalAlignment.Center;
                    cb.HorizontalContentAlignment = HorizontalAlignment.Center;
                    cb.VerticalContentAlignment = VerticalAlignment.Center;
                    cb.FontSize = cb.FontSize * 2;

                    CheckBoxes.Add(cb);
                    assigned++;

                    Grid.SetRow(cb, i);
                    Grid.SetColumn(cb, j);
                    Grid_ProcessConfigEditWindow_CPUSelect.Children.Add(cb);
                }
            }
        }

        private void ComboBox_PrioritySelect_Initialized(object sender, EventArgs e)
        {
            ComboBox_PrioritySelect.ItemsSource = Enum.GetValues(typeof(ProcessPriorityClass)).Cast<ProcessPriorityClass>();
            ComboBox_PrioritySelect.SelectedItem = ProcessPriorityClass.Normal;
        }

        public ProcessConfig GenerateConfig()
        {
            List<int> cores = new List<int>();
            for (int i = 0; i < CheckBoxes.Count; i++)
            {
                if (CheckBoxes[i].IsChecked == true)
                {
                    cores.Add(i);
                }
            }

            ProcessPriorityClass priority = (ProcessPriorityClass)ComboBox_PrioritySelect.SelectedItem ;

            ProcessConfig conf = new ProcessConfig(cores, priority);

            return conf;
        }

        private void CPUSelectCheckBoxChanged(object sender, RoutedEventArgs e)
        {
            if (CheckBoxes.Count(x => x.IsChecked == true) > 0)
            {
                Button_Accept.IsEnabled = true;
                Button_Accept.Tag = "Accept";
            }
            else
            {
                Button_Accept.IsEnabled = false;
                Button_Accept.Tag = "No Threads Selected!";
            }
        }

        private void Button_Accept_Click(object sender, RoutedEventArgs e)
        {
            if (ValidateForm())
            {
                this.DialogResult = true;
                TargetProcessOrGroup = TextBox_TargetProcessOrGroup.Text;
                this.Close();
            }
        }

        private void Button_Cancel_Click(object sender, RoutedEventArgs e)
        {
            this.DialogResult = false;
            this.Close();
        }

        private void TextBox_TargetProcessOrGroup_Loaded(object sender, EventArgs e)
        {
            if (TargetProcessOrGroup != null && TargetProcessOrGroup != "")
            {
                TextBox_TargetProcessOrGroup.Text = TargetProcessOrGroup;
                TextBox_TargetProcessOrGroup.IsEnabled = false;
            }
            else
            {
                TextBox_TargetProcessOrGroup.Text = "";
            }
        }

        private void TextBox_TargetProcessOrGroup_TextChanged(object sender, EventArgs e)
        {
            TargetProcessOrGroup = TextBox_TargetProcessOrGroup.Text;
            ValidateForm();
        }

        private bool ValidateForm()
        {
            if (TextBox_TargetProcessOrGroup.Text != "" && CheckBoxes.Count(x => x.IsChecked == true) > 0)
            {
                Button_Accept.IsEnabled = true;
                return true;
            }
            else
            {
                Button_Accept.IsEnabled = false;
                return false;
            }
        }
    }
}
